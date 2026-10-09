//! Reading a frame out of a real terminal in the two phases of PLAN §4b.

use temor_vt::{
    CellView, CellWidth, FrameDirty, PendingFrame, RenderView, RowFilter, StyleFlags, Terminal,
    Underline,
};

use super::images::ImageCache;
use super::model::{CellDraw, CellText, ImagePlacement, PaneFrame, TermColors};
use crate::error::RenderError;

/// One per terminal: libghostty's render state plus the kitty image copies.
///
/// ```ignore
/// let captured = capture.begin(&mut term.lock())?; // under the pane's lock
/// captured.finish(&mut frame)?;                    // after releasing it
/// renderer.render(&mut frame, &view)?;
/// ```
#[derive(Debug)]
pub struct FrameCapture {
    view: RenderView,
    images: ImageCache,
}

impl FrameCapture {
    pub fn new() -> Result<Self, RenderError> {
        Ok(Self {
            view: RenderView::new()?,
            images: ImageCache::default(),
        })
    }

    /// Phase one, while holding the terminal lock: starts libghostty's update and reads the
    /// kitty placements, which need the terminal itself.
    pub fn begin(&mut self, term: &mut Terminal) -> Result<CapturedFrame<'_>, RenderError> {
        let Self { view, images } = self;
        let pending = view.begin(term)?;
        let images = images.collect(term);
        Ok(CapturedFrame { pending, images })
    }
}

/// A frame begun under the lock; finish it after releasing the lock.
#[derive(Debug)]
pub struct CapturedFrame<'c> {
    pending: PendingFrame<'c>,
    images: Vec<ImagePlacement>,
}

impl CapturedFrame<'_> {
    /// Phase two, without the lock: copies the rows libghostty marks dirty (every row after a
    /// resize or a full redraw) into `frame` and marks them for the renderer.
    pub fn finish(self, frame: &mut PaneFrame) -> Result<(), RenderError> {
        let mut f = self.pending.end()?;
        let size = f.size();
        let resized = size != frame.size;
        frame.resize(size);
        let colors = f.colors();
        frame.set_colors(TermColors {
            foreground: colors.foreground,
            background: colors.background,
            cursor: colors.cursor,
        });
        frame.cursor = f.cursor();
        let dirty = f.dirty();
        if dirty != FrameDirty::Clean || resized {
            let filter = if dirty == FrameDirty::Full || resized {
                RowFilter::All
            } else {
                RowFilter::Dirty
            };
            f.rows(filter, |row| {
                let mut cells = Vec::new();
                // A row whose cells cannot be read draws empty; the next frame retries it.
                let _ = row.cells(|c| {
                    if let Some(cell) = drawable(c) {
                        cells.push(cell);
                    }
                });
                frame.set_row(row.row(), cells);
            })?;
        }
        frame.images = self.images;
        f.mark_drawn()?;
        Ok(())
    }
}

/// The cell as the renderer keeps it, or `None` for spacers and blank default cells.
fn drawable(c: &CellView<'_>) -> Option<CellDraw> {
    let span = match c.width {
        CellWidth::Narrow => 1,
        CellWidth::Wide => 2,
        CellWidth::SpacerTail | CellWidth::SpacerHead => return None,
    };
    let text = CellText::new(c.text);
    let flags = c.style.flags;
    let marked = c.bg.is_some()
        || c.selected == temor_vt::Selected::Yes
        || c.style.underline != Underline::None
        || flags.contains(StyleFlags::INVERSE)
        || flags.contains(StyleFlags::STRIKETHROUGH)
        || flags.contains(StyleFlags::OVERLINE);
    if text.is_blank() && !marked {
        return None;
    }
    Some(CellDraw {
        col: c.col,
        span,
        text,
        fg: c.fg,
        bg: c.bg,
        style: c.style,
        selected: c.selected,
    })
}

//! Reading a frame out of a real terminal in the two phases of PLAN §4b.

use temor_vt::{FrameDirty, PendingFrame, RenderView, RowFilter, Terminal};

use super::images::{Collected, ImageCache};
use super::model::{PaneFrame, TermColors, drawable};
use super::placeholder::PlaceholderCell;
use super::update::Update;
use super::virtual_images::VirtualScene;
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
        let cell = term.cell_px();
        Ok(CapturedFrame {
            pending,
            images,
            cell: (cell.width.0, cell.height.0),
        })
    }
}

/// A frame begun under the lock; finish it after releasing the lock.
#[derive(Debug)]
pub struct CapturedFrame<'c> {
    pending: PendingFrame<'c>,
    images: Collected,
    /// One cell in pixels, as the terminal was told.
    cell: (u32, u32),
}

impl CapturedFrame<'_> {
    /// Phase two, without the lock: copies the rows libghostty marks dirty (every row after a
    /// resize or a full redraw) into `frame` and marks them for the renderer. The frame changes
    /// only when every row was read: a capture that fails after a resize leaves the old size and
    /// rows, and libghostty keeps its dirty marks, so the next capture reads everything again.
    pub fn finish(self, frame: &mut PaneFrame) -> Result<(), RenderError> {
        let mut f = self.pending.end()?;
        let size = f.size();
        let resized = size != frame.size;
        let colors = f.colors();
        let cursor = f.cursor();
        let dirty = f.dirty();
        let mut rows = Vec::new();
        let mut placeholders = Vec::new();
        if dirty != FrameDirty::Clean || resized {
            let filter = if dirty == FrameDirty::Full || resized {
                RowFilter::All
            } else {
                RowFilter::Dirty
            };
            f.rows(filter, |row| {
                let mut cells = Vec::new();
                let mut marks = Vec::new();
                let read = row.cells(|c| {
                    let (fg, underline) = (c.style.fg_ref, c.style.underline_ref);
                    if let Some(mark) = PlaceholderCell::decode(c.col, c.text, fg, underline) {
                        marks.push(mark);
                    } else if let Some(cell) = drawable(c) {
                        cells.push(cell);
                    }
                });
                let at = row.row();
                placeholders.push((at, marks));
                rows.push(read.map(|()| (at, cells)));
            })?;
        }
        let update = Update {
            size,
            colors: TermColors {
                foreground: colors.foreground,
                background: colors.background,
                cursor: colors.cursor,
            },
            cursor,
            images: self.images.direct,
            placeholders,
            virtuals: VirtualScene {
                targets: self.images.virtuals,
                cell: self.cell,
            },
        };
        // Not marking the frame drawn when a row failed keeps libghostty's dirty flags.
        frame.apply(update, rows)?;
        f.mark_drawn()?;
        Ok(())
    }
}

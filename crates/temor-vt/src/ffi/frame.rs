//! `Frame`: a finished render update, read row by row.

use temor_vt_sys as sys;

use super::convert::{check, rgb};
use super::render::RenderView;
use super::row::RowView;
use crate::{
    Col, CursorShape, CursorView, FrameColors, FrameDirty, GridSize, Row, RowFilter, VtError,
};

/// A finished update, ready to draw.
pub struct Frame<'v> {
    view: &'v mut RenderView,
    colors: sys::RenderStateColors,
}

impl std::fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl<'v> Frame<'v> {
    pub(super) fn new(view: &'v mut RenderView, colors: sys::RenderStateColors) -> Self {
        Self { view, colors }
    }

    /// How much changed since the last [`Frame::mark_drawn`].
    pub fn dirty(&self) -> FrameDirty {
        let mut dirty = sys::RenderStateDirty::FULL;
        // SAFETY: DIRTY writes a `GhosttyRenderStateDirty`.
        unsafe {
            sys::ghostty_render_state_get(
                self.view.state(),
                sys::RenderStateData::DIRTY,
                (&raw mut dirty).cast(),
            )
        };
        match dirty {
            sys::RenderStateDirty::FALSE => FrameDirty::Clean,
            sys::RenderStateDirty::PARTIAL => FrameDirty::Partial,
            _ => FrameDirty::Full,
        }
    }

    /// The viewport size in cells.
    pub fn size(&self) -> GridSize {
        let (mut cols, mut rows) = (0u16, 0u16);
        // SAFETY: COLS and ROWS write a `uint16_t`.
        unsafe {
            sys::ghostty_render_state_get(
                self.view.state(),
                sys::RenderStateData::COLS,
                (&raw mut cols).cast(),
            );
            sys::ghostty_render_state_get(
                self.view.state(),
                sys::RenderStateData::ROWS,
                (&raw mut rows).cast(),
            );
        }
        GridSize::new(cols, rows)
    }

    /// The cursor, when it is visible and inside the viewport.
    pub fn cursor(&self) -> Option<CursorView> {
        let mut c = temor_vt_sys::sized!(sys::RenderStateCursor);
        // SAFETY: CURSOR writes a sized `GhosttyRenderStateCursor`.
        let code = unsafe {
            sys::ghostty_render_state_get(
                self.view.state(),
                sys::RenderStateData::CURSOR,
                (&raw mut c).cast(),
            )
        };
        if code != sys::Result::SUCCESS || !c.visible || !c.viewport_has_value {
            return None;
        }
        let shape = match c.visual_style {
            sys::RenderStateCursorVisualStyle::BAR => CursorShape::Bar,
            sys::RenderStateCursorVisualStyle::UNDERLINE => CursorShape::Underline,
            sys::RenderStateCursorVisualStyle::BLOCK_HOLLOW => CursorShape::BlockHollow,
            _ => CursorShape::Block,
        };
        Some(CursorView {
            col: Col(c.viewport_x),
            row: Row(c.viewport_y),
            shape,
            blinking: c.blinking,
            wide_tail: c.wide_tail,
        })
    }

    /// Default colours and the palette.
    pub fn colors(&self) -> FrameColors {
        FrameColors {
            foreground: rgb(self.colors.foreground),
            background: rgb(self.colors.background),
            cursor: self
                .colors
                .cursor_has_value
                .then(|| rgb(self.colors.cursor)),
            palette: self.view.palette.clone(),
        }
    }

    /// Calls `f` for each row `filter` selects, top to bottom.
    pub fn rows(
        &mut self,
        filter: RowFilter,
        mut f: impl FnMut(&mut RowView<'_>),
    ) -> Result<(), VtError> {
        let view = &mut *self.view;
        let mut iter = view.rows.as_ptr();
        // SAFETY: ROW_ITERATOR fills the view's preallocated iterator from the finished state.
        check(
            unsafe {
                sys::ghostty_render_state_get(
                    view.state(),
                    sys::RenderStateData::ROW_ITERATOR,
                    (&raw mut iter).cast(),
                )
            },
            "rows",
        )?;
        // SAFETY: the iterator was just populated from a state that is not updated while it lives.
        while unsafe { sys::ghostty_render_state_row_iterator_next(iter) } {
            let mut row = RowView::new(iter, view.cells.as_ptr(), &mut view.text, &view.palette);
            if filter == RowFilter::Dirty && !row.dirty() {
                continue;
            }
            f(&mut row);
        }
        Ok(())
    }

    /// Marks the frame and every row clean; call after drawing all of it.
    pub fn mark_drawn(self) -> Result<(), VtError> {
        // SAFETY: the state is live.
        check(
            unsafe { sys::ghostty_render_state_clean(self.view.state()) },
            "clean",
        )
    }
}

//! `RenderView`: the renderer's copy of a terminal, updated in two phases so the terminal lock is
//! only held for the first.

use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::convert::{check, rgb};
use super::row::RowView;
use super::terminal::Terminal;
use crate::{
    Col, CursorBlink, CursorCell, CursorShape, CursorView, FrameColors, FrameDirty, GridSize, Rgb,
    RowFilter, VtError,
};

/// A render state with its row and cell cursors and scratch space, reused frame after frame.
///
/// Per frame: [`RenderView::begin`] while holding the terminal lock, drop the lock,
/// [`PendingFrame::end`], read the [`Frame`], then [`Frame::mark_drawn`].
pub struct RenderView {
    state: NonNull<sys::RenderStateImpl>,
    rows: NonNull<sys::RenderStateRowIteratorImpl>,
    cells: NonNull<sys::RenderStateRowCellsImpl>,
    pub(super) text: Vec<u8>,
    pub(super) palette: Vec<Rgb>,
}

// SAFETY: a `RenderView` owns plain memory inside libghostty with no thread-local state and no
// callbacks; it is never shared (`!Sync`), so moving it between threads is sound.
unsafe impl Send for RenderView {}

impl std::fmt::Debug for RenderView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderView").finish_non_exhaustive()
    }
}

impl RenderView {
    /// An empty view; the first frame after it is fully dirty.
    pub fn new() -> Result<Self, VtError> {
        let null = std::ptr::null();
        let (mut state, mut rows, mut cells) = (
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        // SAFETY: valid out-pointers; a null allocator is the default one. Partially created
        // handles are freed below if a later step fails.
        let made = unsafe {
            check(
                sys::ghostty_render_state_new(null, &mut state),
                "render state",
            )
            .and_then(|()| {
                check(
                    sys::ghostty_render_state_row_iterator_new(null, &mut rows),
                    "rows",
                )
            })
            .and_then(|()| {
                check(
                    sys::ghostty_render_state_row_cells_new(null, &mut cells),
                    "cells",
                )
            })
        };
        match (
            made,
            NonNull::new(state),
            NonNull::new(rows),
            NonNull::new(cells),
        ) {
            (Ok(()), Some(state), Some(rows), Some(cells)) => Ok(Self {
                state,
                rows,
                cells,
                text: Vec::with_capacity(64),
                palette: vec![Rgb::default(); 256],
            }),
            (made, ..) => {
                // SAFETY: each free accepts null.
                unsafe {
                    sys::ghostty_render_state_row_cells_free(cells);
                    sys::ghostty_render_state_row_iterator_free(rows);
                    sys::ghostty_render_state_free(state);
                }
                Err(made.err().unwrap_or(VtError::OutOfMemory))
            }
        }
    }

    /// Phase one: copies what changed out of `term`. Hold the terminal lock only for this call.
    pub fn begin(&mut self, term: &mut Terminal) -> Result<PendingFrame<'_>, VtError> {
        // SAFETY: both handles are live and the caller has exclusive access to the terminal.
        check(
            unsafe { sys::ghostty_render_state_begin_update(self.state.as_ptr(), term.raw()) },
            "update",
        )?;
        Ok(PendingFrame { view: Some(self) })
    }

    pub(super) fn state(&self) -> sys::RenderState {
        self.state.as_ptr()
    }
}

impl Drop for RenderView {
    fn drop(&mut self) {
        // SAFETY: the handles were made in `new` and are freed once.
        unsafe {
            sys::ghostty_render_state_row_cells_free(self.cells.as_ptr());
            sys::ghostty_render_state_row_iterator_free(self.rows.as_ptr());
            sys::ghostty_render_state_free(self.state.as_ptr());
        }
    }
}

/// A begun update. Finish it with [`PendingFrame::end`] after releasing the terminal lock;
/// dropping it finishes it too.
#[derive(Debug)]
pub struct PendingFrame<'v> {
    view: Option<&'v mut RenderView>,
}

impl<'v> PendingFrame<'v> {
    /// Phase two: finishes the update without touching the terminal.
    pub fn end(mut self) -> Result<Frame<'v>, VtError> {
        let view = self
            .view
            .take()
            .ok_or(VtError::InvalidValue("frame already ended"))?;
        // SAFETY: the state is live; end only touches the state's own memory.
        check(
            unsafe { sys::ghostty_render_state_end_update(view.state()) },
            "update",
        )?;
        let mut colors = temor_vt_sys::sized!(sys::RenderStateColors);
        // SAFETY: COLORS writes a sized `GhosttyRenderStateColors`.
        check(
            unsafe {
                sys::ghostty_render_state_get(
                    view.state(),
                    sys::RenderStateData::COLORS,
                    (&raw mut colors).cast(),
                )
            },
            "colors",
        )?;
        view.palette = colors.palette.iter().copied().map(rgb).collect();
        Ok(Frame { view, colors })
    }
}

impl Drop for PendingFrame<'_> {
    fn drop(&mut self) {
        if let Some(view) = self.view.take() {
            // SAFETY: completing a begun update is always allowed.
            unsafe { sys::ghostty_render_state_end_update(view.state()) };
        }
    }
}

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

impl Frame<'_> {
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
            row: c.viewport_y,
            shape,
            blink: if c.blinking {
                CursorBlink::Blinking
            } else {
                CursorBlink::Steady
            },
            cell: if c.wide_tail {
                CursorCell::WideTail
            } else {
                CursorCell::Normal
            },
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
            if filter == RowFilter::Dirty && row.dirty() == crate::RowDirty::Clean {
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

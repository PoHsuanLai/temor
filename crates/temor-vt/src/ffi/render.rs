//! `RenderView`: the renderer's copy of a terminal, updated in two phases so the terminal lock is
//! only held for the first.

use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::convert::{check, rgb};
use super::frame::Frame;
use super::terminal::Terminal;
use crate::{Rgb, VtError};

/// A render state with its row and cell cursors and scratch space, reused frame after frame.
///
/// Per frame: [`RenderView::begin`] while holding the terminal lock, drop the lock,
/// [`PendingFrame::end`], read the [`Frame`], then [`Frame::mark_drawn`].
///
/// Keep **one `RenderView` per terminal**: dirty tracking is consumed from the terminal by
/// `begin`, so a second view on the same terminal (or one view alternating between terminals)
/// sees rows as clean that it never drew.
pub struct RenderView {
    state: NonNull<sys::RenderStateImpl>,
    pub(super) rows: NonNull<sys::RenderStateRowIteratorImpl>,
    pub(super) cells: NonNull<sys::RenderStateRowCellsImpl>,
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
        Ok(Frame::new(view, colors))
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

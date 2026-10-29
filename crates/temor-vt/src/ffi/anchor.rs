//! Anchors (PLAN §8.1, §9): libghostty's tracked grid references, one per shell mark. A tracked
//! reference follows its cell through scrolling, pruning and reflow, and loses its value once
//! the scrollback drops the cell. Anchors are owned here and handed out as ids.

use std::collections::BTreeMap;
use std::ffi::c_void;

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{AnchorId, Col, GridPoint, PointSpace};

/// Most anchors kept at once. A new one past this releases the oldest.
pub(super) const MAX_ANCHORS: usize = 4096;

/// The live anchors of one terminal. Freed by the terminal before the terminal itself.
#[derive(Debug, Default)]
pub(super) struct Anchors {
    next: u64,
    live: BTreeMap<u64, sys::TrackedGridRef>,
}

impl Anchors {
    /// Frees every reference.
    ///
    /// # Safety
    /// Must run while the terminal the references came from is still live or has never been
    /// used through them again; called from the terminal's `Drop` before it is freed.
    pub(super) unsafe fn free_all(&mut self) {
        for (_, tracked) in std::mem::take(&mut self.live) {
            // SAFETY: each reference was created by `ghostty_terminal_grid_ref_track` and is
            // freed exactly once, here (it was removed from the map).
            unsafe { sys::ghostty_tracked_grid_ref_free(tracked) };
        }
    }

    /// Frees one reference, if it is live.
    fn release(&mut self, id: AnchorId) {
        if let Some(tracked) = self.live.remove(&id.0) {
            // SAFETY: the reference was removed from the map, so it is freed exactly once.
            unsafe { sys::ghostty_tracked_grid_ref_free(tracked) };
        }
    }
}

fn tag_of(space: PointSpace) -> sys::PointTag::Type {
    match space {
        PointSpace::Active => sys::PointTag::ACTIVE,
        PointSpace::Viewport => sys::PointTag::VIEWPORT,
        PointSpace::Screen => sys::PointTag::SCREEN,
    }
}

impl Terminal {
    /// Anchors the cursor's cell on the active screen. `None` when libghostty refuses the point.
    pub fn anchor_cursor(&mut self) -> Option<AnchorId> {
        let (col, row) = (
            self.cursor_u16(sys::TerminalData::CURSOR_X)?,
            self.cursor_u16(sys::TerminalData::CURSOR_Y)?,
        );
        self.anchor_at(GridPoint::new(PointSpace::Active, col, u32::from(row)))
    }

    /// Anchors the cell at `at`. `None` when libghostty refuses the point.
    pub fn anchor_at(&mut self, at: GridPoint) -> Option<AnchorId> {
        let mut value = sys::PointValue::default();
        value.coordinate = sys::PointCoordinate {
            x: at.col.0,
            y: at.row,
        };
        let point = sys::Point {
            tag: tag_of(at.space),
            value,
        };
        let mut tracked: sys::TrackedGridRef = std::ptr::null_mut();
        // SAFETY: the terminal is live and `tracked` is a valid out-pointer.
        check(
            unsafe { sys::ghostty_terminal_grid_ref_track(self.raw(), point, &mut tracked) },
            "anchor",
        )
        .ok()?;
        let anchors = self.anchors_mut();
        let id = AnchorId(anchors.next);
        anchors.next += 1;
        if anchors.live.len() >= MAX_ANCHORS {
            let oldest = anchors.live.keys().next().copied();
            if let Some(oldest) = oldest {
                anchors.release(AnchorId(oldest));
            }
        }
        anchors.live.insert(id.0, tracked);
        Some(id)
    }

    /// Where an anchor is now, in `space`. `None` once the scrollback has dropped its cell, the
    /// anchor was released, or the point cannot be shown in `space`.
    pub fn anchor_point(&self, id: AnchorId, space: PointSpace) -> Option<GridPoint> {
        let tracked = *self.anchors().live.get(&id.0)?;
        let mut at = sys::PointCoordinate::default();
        // SAFETY: `tracked` is live (it is in the map and freed only by it); `at` is a valid
        // out-pointer for a point coordinate.
        let code = unsafe { sys::ghostty_tracked_grid_ref_point(tracked, tag_of(space), &mut at) };
        (code == sys::Result::SUCCESS).then_some(GridPoint {
            space,
            col: Col(at.x),
            row: at.y,
        })
    }

    /// The anchors whose rows are in the scrollback of the screen in use, the rows `CSI 3 J`
    /// erases. None while the alternate screen is in use: it has no scrollback.
    pub(super) fn anchors_in_history(&self) -> Vec<AnchorId> {
        if self.on_alternate_screen() {
            return Vec::new();
        }
        self.anchors()
            .live
            .keys()
            .map(|&id| AnchorId(id))
            .filter(|&id| {
                self.anchor_point(id, PointSpace::Screen).is_some()
                    && self.anchor_point(id, PointSpace::Active).is_none()
            })
            .collect()
    }

    /// The anchors whose rows are on the screen in use.
    pub(super) fn anchors_on_screen(&self) -> Vec<AnchorId> {
        self.anchors()
            .live
            .keys()
            .map(|&id| AnchorId(id))
            .filter(|&id| self.anchor_point(id, PointSpace::Active).is_some())
            .collect()
    }

    /// Erases the scrollback from `row` of the screen space down, keeping the rows above it and
    /// the screen. Anchors in the erased rows end.
    pub(super) fn erase_history_from(&mut self, row: u32) {
        let mut value = sys::PointValue::default();
        value.coordinate = sys::PointCoordinate { x: 0, y: row };
        let point = sys::Point {
            tag: sys::PointTag::SCREEN,
            value,
        };
        // SAFETY: the terminal is live; the call only reads the point.
        let code = unsafe { sys::ghostty_terminal_erase_history_from(self.raw(), point) };
        // An out-of-range row erases nothing, which is the most that can be done with it.
        let _ = check(code, "erase the scrollback from a row");
    }

    /// Moves a live anchor to the screen's top-left cell. Nothing happens to one already released.
    pub(super) fn move_anchor_to_top(&mut self, id: AnchorId) {
        let Some(&tracked) = self.anchors().live.get(&id.0) else {
            return;
        };
        let point = sys::Point {
            tag: sys::PointTag::ACTIVE,
            value: sys::PointValue::default(),
        };
        // SAFETY: `tracked` is live (it is in the map) and belongs to this terminal.
        let code = unsafe { sys::ghostty_tracked_grid_ref_set(tracked, self.raw(), point) };
        let _ = check(code, "move an anchor");
    }

    /// The cursor's row on the active screen, now (row 0 is the active screen's top), without
    /// waiting for a frame. `None` when libghostty will not say.
    pub fn cursor_row(&self) -> Option<u16> {
        self.cursor_u16(sys::TerminalData::CURSOR_Y)
    }

    /// How many anchors are live.
    pub fn anchor_count(&self) -> usize {
        self.anchors().live.len()
    }

    /// Numbers this terminal's anchors after every anchor `earlier` handed out. A terminal that
    /// replaces another (a screen restored from a snapshot) then never reuses an id the window may
    /// still hold from the old one, so releasing that old id cannot free one of the new anchors.
    pub fn number_anchors_after(&mut self, earlier: &Terminal) {
        let next = earlier.anchors().next;
        let anchors = self.anchors_mut();
        anchors.next = anchors.next.max(next);
    }

    /// Frees an anchor. Releasing one twice, or one that never existed, does nothing.
    pub fn release_anchor(&mut self, id: AnchorId) {
        self.anchors_mut().release(id);
    }

    fn cursor_u16(&self, data: sys::TerminalData::Type) -> Option<u16> {
        let mut value: u16 = 0;
        // SAFETY: the terminal is live; the cursor data writes a `uint16_t`.
        let code = unsafe {
            sys::ghostty_terminal_get(self.raw(), data, (&raw mut value).cast::<c_void>())
        };
        (code == sys::Result::SUCCESS).then_some(value)
    }
}

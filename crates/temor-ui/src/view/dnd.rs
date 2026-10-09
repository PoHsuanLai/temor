//! Dragging in the sidebar (PLAN §7): a tab row dropped on another row joins its tab, a tile of a
//! row's miniature dropped outside the sidebar tears its pane off into a tab of its own. The drag
//! machine and the ghost are quire's (`use_drag`, `DragGhost`, `DropLine`); this holds where the
//! rows are and the rule for what a release means. The rule is pure.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::motion::drag::{DragPhase, DragTracker};
use ds::prelude::{Point, Px, Rect};
use temor_core::vocab::ids::{PaneId, TabId};

use crate::hub::DragKey;

/// Where each tab row is on the page, as the rows report it. Two are equal when they are the same.
#[derive(Clone, Default)]
pub struct RowRects(Rc<RefCell<BTreeMap<TabId, Rect>>>);

impl PartialEq for RowRects {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for RowRects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RowRects")
    }
}

impl RowRects {
    /// A row is at `rect`.
    pub fn set(&self, tab: TabId, rect: Rect) {
        self.0.borrow_mut().insert(tab, rect);
    }

    /// A row left the page.
    pub fn forget(&self, tab: TabId) {
        self.0.borrow_mut().remove(&tab);
    }

    /// The row under `at`.
    pub fn at(&self, at: Point) -> Option<TabId> {
        self.0
            .borrow()
            .iter()
            .find(|(_, r)| {
                at.x.0 >= r.left().0
                    && at.x.0 < r.right().0
                    && at.y.0 >= r.top().0
                    && at.y.0 < r.bottom().0
            })
            .map(|(tab, _)| *tab)
    }
}

/// Whether the click that follows a drag's release still counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Counts,
    Swallowed,
}

/// Everything the sidebar's drag shares: the machine, the rows' places, the row under the pointer
/// and the click a finished drag swallows.
#[derive(Clone, PartialEq)]
pub struct Dnd {
    pub drag: DragTracker<DragKey>,
    pub rects: RowRects,
    pub over: Signal<Option<TabId>>,
    pub click: ClickCell,
}

impl std::fmt::Debug for Dnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dnd").finish_non_exhaustive()
    }
}

/// The click flag, shared and equal only to itself.
#[derive(Clone, Default)]
pub struct ClickCell(Rc<Cell<Option<Click>>>);

impl PartialEq for ClickCell {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ClickCell {
    /// Marks the next click as swallowed (a drag just finished).
    pub fn swallow(&self) {
        self.0.set(Some(Click::Swallowed));
    }

    /// A press starts: its click counts.
    pub fn reset(&self) {
        self.0.set(Some(Click::Counts));
    }

    /// Whether the click that is happening counts; asking resets it.
    pub fn take(&self) -> Click {
        self.0.replace(Some(Click::Counts)).unwrap_or(Click::Counts)
    }
}

/// What a release means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// Tab `from` joins the tab `onto`.
    Join { from: TabId, onto: TabId },
    /// The pane leaves its tab for one of its own.
    Tear(PaneId),
    /// Nothing happens.
    Nothing,
}

/// What releasing a drag in `phase` means, with `over` the row under the pointer and
/// `sidebar_right` the sidebar's right edge (when it is known). A tab dropped on another row
/// joins it; a tile dropped on no row and past the sidebar's edge tears off.
pub fn resolve(phase: &DragPhase<DragKey>, over: Option<TabId>, sidebar_right: Option<Px>) -> Dropped {
    match phase {
        DragPhase::Live {
            key: DragKey::Tab(from),
            ..
        } => match over {
            Some(onto) if onto != *from => Dropped::Join { from: *from, onto },
            _ => Dropped::Nothing,
        },
        DragPhase::Live {
            key: DragKey::Tile(pane),
            at,
            ..
        } => match (over, sidebar_right) {
            (None, Some(edge)) if at.x.0 > edge.0 => Dropped::Tear(*pane),
            _ => Dropped::Nothing,
        },
        DragPhase::Idle | DragPhase::Pending { .. } => Dropped::Nothing,
    }
}

/// A pointer event's place on the page.
pub fn point_of(event: &PointerEvent) -> Point {
    let at = event.client_coordinates();
    Point {
        x: Px(at.x as f32),
        y: Px(at.y as f32),
    }
}

/// Whether the press is the primary button's (the only one that drags).
pub fn is_primary(event: &PointerEvent) -> bool {
    matches!(event.trigger_button(), None | Some(MouseButton::Primary))
}

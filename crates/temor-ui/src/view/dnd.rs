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
pub fn resolve(
    phase: &DragPhase<DragKey>,
    over: Option<TabId>,
    sidebar_right: Option<Px>,
) -> Dropped {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn live(key: DragKey, x: f32) -> DragPhase<DragKey> {
        DragPhase::Live {
            key,
            at: Point {
                x: Px(x),
                y: Px(10.0),
            },
            target: None,
        }
    }

    #[test]
    fn a_release_joins_tears_or_does_nothing() {
        let (a, b) = (TabId(1), TabId(2));
        let pane = PaneId(7);
        let edge = Some(Px(200.0));
        #[rustfmt::skip]
        let rows: &[(&str, DragPhase<DragKey>, Option<TabId>, Option<Px>, Dropped)] = &[
            ("a tab over another row", live(DragKey::Tab(a), 50.0), Some(b), edge, Dropped::Join { from: a, onto: b }),
            ("a tab over its own row", live(DragKey::Tab(a), 50.0), Some(a), edge, Dropped::Nothing),
            ("a tab over no row", live(DragKey::Tab(a), 50.0), None, edge, Dropped::Nothing),
            ("a tile past the sidebar", live(DragKey::Tile(pane), 300.0), None, edge, Dropped::Tear(pane)),
            ("a tile inside the sidebar", live(DragKey::Tile(pane), 100.0), None, edge, Dropped::Nothing),
            ("a tile over a row", live(DragKey::Tile(pane), 300.0), Some(b), edge, Dropped::Nothing),
            ("a tile when the sidebar is not measured", live(DragKey::Tile(pane), 300.0), None, None, Dropped::Nothing),
            ("a press that never became a drag", DragPhase::Pending { key: DragKey::Tab(a), from: Point { x: Px(0.0), y: Px(0.0) } }, Some(b), edge, Dropped::Nothing),
        ];
        for (name, phase, over, edge, want) in rows {
            assert_eq!(resolve(phase, *over, *edge), *want, "{name}");
        }
    }

    #[test]
    fn the_row_under_a_point_is_found_and_a_row_that_left_is_forgotten() {
        let rects = RowRects::default();
        let rect = |top: f32| Rect {
            origin: Point {
                x: Px(0.0),
                y: Px(top),
            },
            size: ds::prelude::Size {
                width: Px(100.0),
                height: Px(20.0),
            },
        };
        rects.set(TabId(1), rect(0.0));
        rects.set(TabId(2), rect(20.0));
        let at = |y: f32| Point {
            x: Px(5.0),
            y: Px(y),
        };
        assert_eq!(rects.at(at(5.0)), Some(TabId(1)));
        assert_eq!(rects.at(at(20.0)), Some(TabId(2)));
        assert_eq!(rects.at(at(45.0)), None);
        rects.forget(TabId(2));
        assert_eq!(rects.at(at(25.0)), None);
    }
}

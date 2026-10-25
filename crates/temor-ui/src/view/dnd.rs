//! Dragging in the sidebar (PLAN §7): a tab row dropped on another row joins its tab, a pane's
//! segment dropped outside the sidebar tears it off into a tab of its own, pinned tabs move as
//! Arc's do (a pinned row dropped on another takes its place, on a loose row it is unpinned there,
//! and a loose row dropped on a pinned one is pinned there), and a row dropped on the Favourites
//! keeps its folder there. The drag
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

/// What a drag can be let go over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A tab's row.
    Tab(TabId),
    /// The Favourites, at the top of the sidebar.
    Favourites,
}

/// Where each tab row is on the page, and the Favourites, as they report it. Two are equal when
/// they are the same.
#[derive(Clone, Default)]
pub struct RowRects(Rc<RefCell<Places>>);

#[derive(Default)]
struct Places {
    rows: BTreeMap<TabId, Rect>,
    favourites: Option<Rect>,
}

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
        self.0.borrow_mut().rows.insert(tab, rect);
    }

    /// A row left the page.
    pub fn forget(&self, tab: TabId) {
        self.0.borrow_mut().rows.remove(&tab);
    }

    /// The Favourites are at `rect`, or (`None`) not on the page.
    pub fn set_favourites(&self, rect: Option<Rect>) {
        self.0.borrow_mut().favourites = rect;
    }

    /// What is under `at`: the Favourites, or a row.
    pub fn at(&self, at: Point) -> Option<Target> {
        let inside = |r: &Rect| {
            at.x.0 >= r.left().0
                && at.x.0 < r.right().0
                && at.y.0 >= r.top().0
                && at.y.0 < r.bottom().0
        };
        let places = self.0.borrow();
        if places.favourites.as_ref().is_some_and(inside) {
            return Some(Target::Favourites);
        }
        places
            .rows
            .iter()
            .find(|(_, r)| inside(r))
            .map(|(tab, _)| Target::Tab(*tab))
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
    pub over: Signal<Option<Target>>,
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

impl std::fmt::Debug for ClickCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ClickCell").finish_non_exhaustive()
    }
}

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

    /// Runs `act` for a click that counts: not the one that follows a drag's release.
    pub fn counted(&self, act: impl FnOnce()) {
        if self.take() == Click::Counts {
            act();
        }
    }
}

/// What a release means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// Tab `from` joins the tab `onto`.
    Join { from: TabId, onto: TabId },
    /// Pinned tab `from` moves to where pinned tab `onto` stands.
    Place { from: TabId, onto: TabId },
    /// Pinned tab `from` is unpinned where the tab `onto` stands.
    Unpin { from: TabId, onto: TabId },
    /// Tab `from` is pinned where the pinned tab `onto` stands.
    Pin { from: TabId, onto: TabId },
    /// The pane leaves its tab for one of its own.
    Tear(PaneId),
    /// Tab `from`'s folder is kept among the Favourites.
    Favourite(TabId),
    /// Nothing happens.
    Nothing,
}

/// What releasing a drag in `phase` means, with `over` what is under the pointer,
/// `sidebar_right` the sidebar's right edge (when it is known) and `pinned` whether a tab is
/// pinned. A tab dropped on another row joins it; pinned tabs move, pin or unpin instead (a pinned
/// tab keeps its panes); a tab dropped on the Favourites is kept there; a segment dropped on
/// nothing and past the sidebar's edge tears off.
pub fn resolve(
    phase: &DragPhase<DragKey>,
    over: Option<Target>,
    sidebar_right: Option<Px>,
    pinned: impl Fn(TabId) -> bool,
) -> Dropped {
    match phase {
        DragPhase::Live {
            key: DragKey::Tab(from),
            ..
        } => match over {
            Some(Target::Favourites) => Dropped::Favourite(*from),
            Some(Target::Tab(onto)) if onto != *from => {
                let from = *from;
                match (pinned(from), pinned(onto)) {
                    (true, true) => Dropped::Place { from, onto },
                    (true, false) => Dropped::Unpin { from, onto },
                    (false, true) => Dropped::Pin { from, onto },
                    (false, false) => Dropped::Join { from, onto },
                }
            }
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
        #[allow(clippy::type_complexity)] // a table of named rows
        let rows: &[(&str, DragPhase<DragKey>, Option<Target>, Option<Px>, Dropped)] = &[
            ("a tab over another row", live(DragKey::Tab(a), 50.0), Some(Target::Tab(b)), edge, Dropped::Join { from: a, onto: b }),
            ("a tab over its own row", live(DragKey::Tab(a), 50.0), Some(Target::Tab(a)), edge, Dropped::Nothing),
            ("a tab over the Favourites", live(DragKey::Tab(a), 50.0), Some(Target::Favourites), edge, Dropped::Favourite(a)),
            ("a tab over no row", live(DragKey::Tab(a), 50.0), None, edge, Dropped::Nothing),
            ("a tile past the sidebar", live(DragKey::Tile(pane), 300.0), None, edge, Dropped::Tear(pane)),
            ("a tile inside the sidebar", live(DragKey::Tile(pane), 100.0), None, edge, Dropped::Nothing),
            ("a tile over a row", live(DragKey::Tile(pane), 300.0), Some(Target::Tab(b)), edge, Dropped::Nothing),
            ("a tile over the Favourites", live(DragKey::Tile(pane), 50.0), Some(Target::Favourites), edge, Dropped::Nothing),
            ("a tile when the sidebar is not measured", live(DragKey::Tile(pane), 300.0), None, None, Dropped::Nothing),
            ("a press that never became a drag", DragPhase::Pending { key: DragKey::Tab(a), from: Point { x: Px(0.0), y: Px(0.0) } }, Some(Target::Tab(b)), edge, Dropped::Nothing),
        ];
        for (name, phase, over, edge, want) in rows {
            assert_eq!(resolve(phase, *over, *edge, |_| false), *want, "{name}");
        }
    }

    #[test]
    fn pinned_tabs_move_pin_and_unpin_as_arcs_do() {
        let (p, q, l, m) = (TabId(1), TabId(2), TabId(3), TabId(4));
        let pinned = |tab: TabId| tab == p || tab == q;
        let edge = Some(Px(200.0));
        #[rustfmt::skip]
        let rows: &[(&str, TabId, TabId, Dropped)] = &[
            ("a pinned row on another", p, q, Dropped::Place { from: p, onto: q }),
            ("a pinned row on a loose one", p, l, Dropped::Unpin { from: p, onto: l }),
            ("a loose row on a pinned one", l, q, Dropped::Pin { from: l, onto: q }),
            ("a row on a row", l, m, Dropped::Join { from: l, onto: m }),
        ];
        for (name, from, onto, want) in rows {
            let phase = live(DragKey::Tab(*from), 50.0);
            assert_eq!(
                resolve(&phase, Some(Target::Tab(*onto)), edge, pinned),
                *want,
                "{name}"
            );
        }
    }

    #[test]
    fn what_is_under_a_point_is_found_and_what_left_is_forgotten() {
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
        assert_eq!(rects.at(at(5.0)), Some(Target::Tab(TabId(1))));
        assert_eq!(rects.at(at(20.0)), Some(Target::Tab(TabId(2))));
        assert_eq!(rects.at(at(45.0)), None);
        rects.forget(TabId(2));
        assert_eq!(rects.at(at(25.0)), None);
        rects.set_favourites(Some(rect(40.0)));
        assert_eq!(rects.at(at(45.0)), Some(Target::Favourites));
        rects.set_favourites(None);
        assert_eq!(rects.at(at(45.0)), None);
    }
}

//! The pointer over a pane's grid (PLAN §6, §10): the one handler set that feeds the peek, the
//! link under the pointer and the block menu from the same events. A press or leaving puts the
//! preview away.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::host::measure::RectProbe;
use ds::host::signals::HostSignals;

use super::block_menu::BlockMenuAt;
use super::handlers::{block_menu_opened, grid_pressed, pointer_over};
use super::peek::PeekHandles;
use super::slot::Slot;
use crate::session::Session;

/// What the grid element is given.
pub struct GridPointer {
    pub moved: Box<dyn FnMut(PointerEvent)>,
    pub pressed: Box<dyn FnMut(PointerEvent)>,
    pub left: Box<dyn FnMut(PointerEvent)>,
    /// A right-click's `contextmenu`: the block menu.
    pub context: Box<dyn FnMut(MouseEvent)>,
}

impl std::fmt::Debug for GridPointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridPointer").finish_non_exhaustive()
    }
}

/// The pane's state the grid's pointer sets.
#[derive(Debug, Clone, Copy)]
pub struct GridState {
    /// Bumped to draw the pane again.
    pub ui: Signal<u32>,
    pub notice: Signal<Option<String>>,
    /// Where the block menu is open.
    pub menu: Signal<Option<BlockMenuAt>>,
    /// The pane's region, which pointer events are placed against.
    pub rect: RectProbe,
}

/// The grid's pointer handlers over the pane's `slot`. `mouse_down` is the pane's own press.
pub fn grid_pointer(
    slot: &Rc<RefCell<Slot>>,
    peek: &PeekHandles,
    signals: Option<HostSignals>,
    state: GridState,
    mouse_down: Rc<dyn Fn(PointerEvent)>,
) -> GridPointer {
    let GridState {
        ui,
        notice,
        menu,
        rect,
    } = state;
    let (moved, mut hover) = (
        peek.moved.clone(),
        pointer_over(Rc::clone(slot), (rect, signals)),
    );
    let (gone, mut press) = (
        peek.gone.clone(),
        grid_pressed(Rc::clone(slot), (rect, signals), ui, notice, mouse_down),
    );
    let leave = peek.gone.clone();
    let context = block_menu_opened(Rc::clone(slot), (rect, signals), ui, menu);
    let (link_slot, link_gone) = (Rc::clone(slot), Rc::clone(slot));
    let (mut link_ui, mut gone_ui) = (ui, ui);
    GridPointer {
        moved: Box::new(move |e: PointerEvent| {
            moved(e.clone());
            hover(e);
            // The status line shows the link under the pointer; draw it again when it changes.
            let changed = link_slot
                .borrow_mut()
                .session
                .as_mut()
                .map(Session::refresh_link);
            if changed == Some(true) {
                link_ui += 1;
            }
        }),
        pressed: Box::new(move |e: PointerEvent| {
            gone();
            press(e);
        }),
        context: Box::new(context),
        left: Box::new(move |_: PointerEvent| {
            leave();
            let changed = link_gone
                .borrow_mut()
                .session
                .as_mut()
                .map(Session::link_left);
            if changed == Some(true) {
                gone_ui += 1;
            }
        }),
    }
}

//! The pointer over a pane's grid (PLAN §6, §10): the one handler set that feeds the peek, the
//! block hover and the gutter press from the same events. A press or leaving puts the preview
//! away.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::host::signals::HostSignals;
use temor_core::vocab::ids::BlockId;

use super::handlers::{block_hovered, gutter_pressed};
use super::peek::PeekHandles;
use super::slot::Slot;

/// What the grid element is given.
pub struct GridPointer {
    pub moved: Box<dyn FnMut(PointerEvent)>,
    pub pressed: Box<dyn FnMut(PointerEvent)>,
    pub left: Box<dyn FnMut(PointerEvent)>,
}

/// The grid's pointer handlers over the pane's `slot`. `mouse_down` is the pane's own press.
pub fn grid_pointer(
    slot: &Rc<RefCell<Slot>>,
    peek: &PeekHandles,
    signals: Option<HostSignals>,
    hovered: Signal<Option<BlockId>>,
    ui: Signal<u32>,
    notice: Signal<Option<String>>,
    mouse_down: Rc<dyn Fn(PointerEvent)>,
) -> GridPointer {
    let (moved, mut hover) = (
        peek.moved.clone(),
        block_hovered(Rc::clone(slot), signals, hovered),
    );
    let (gone, mut press) = (
        peek.gone.clone(),
        gutter_pressed(Rc::clone(slot), signals, ui, notice, mouse_down),
    );
    let leave = peek.gone.clone();
    GridPointer {
        moved: Box::new(move |e: PointerEvent| {
            moved(e.clone());
            hover(e);
        }),
        pressed: Box::new(move |e: PointerEvent| {
            gone();
            press(e);
        }),
        left: Box::new(move |_: PointerEvent| leave()),
    }
}

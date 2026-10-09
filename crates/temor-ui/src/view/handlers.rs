//! The window's event handlers, as functions that return the closure a component hands to its
//! element or control. Each takes the session's slot and the window's signals, and does the
//! session call, then asks the window to draw again.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::base::time::FRAME_SLACK;
use ds::base::time::clock::sleep;
use ds::host::signals::HostSignals;
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_pane::keys::Action;

use super::find_bar::StepDirection;
use super::ime::ImeGlue;
use super::slot::{Slot, repaint};
use super::window_help::{device_at, run_query_later};
use crate::hub::Desk;

/// The find bar's query, run once typing pauses.
pub fn find_query(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
    started: Instant,
) -> impl FnMut(String) + 'static {
    move |text: String| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.queue_query(&text, started.elapsed());
        }
        // The search runs once typing pauses (see `run_query_later`).
        run_query_later(Rc::clone(&slot), started);
        ui += 1;
    }
}

/// The find bar's next or previous match.
pub fn find_step(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
) -> impl FnMut(StepDirection) + 'static {
    move |direction: StepDirection| {
        let action = match direction {
            StepDirection::Forward => Action::FindNext,
            StepDirection::Backward => Action::FindPrevious,
        };
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            let _ = s.run(action);
        }
        ui += 1;
        repaint(&slot);
    }
}

/// The find bar's close. Its field leaves the page with it, so the keyboard goes back to the
/// terminal region (`regain`, the same focus the region takes on a click).
pub fn find_close(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
    ime: ImeGlue,
    regain: EventHandler<()>,
) -> impl FnMut(()) + 'static {
    move |_: ()| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            let _ = s.run(Action::CloseFind);
        }
        ui += 1;
        repaint(&slot);
        ime.take_focus(regain);
    }
}

/// A press in the pane: in the gutter it selects a block; anywhere else it is the grid's.
pub fn gutter_pressed(
    slot: Rc<RefCell<Slot>>,
    signals: Option<HostSignals>,
    mut ui: Signal<u32>,
    mut notice: Signal<Option<String>>,
    mouse_down: Rc<dyn Fn(PointerEvent)>,
) -> impl FnMut(PointerEvent) + 'static {
    move |e: PointerEvent| {
        let at = device_at(&e, signals);
        let gutter = match slot.borrow_mut().session.as_mut() {
            Some(s) => s.gutter_press(at),
            None => Ok(false),
        };
        match gutter {
            Ok(true) => {
                ui += 1;
                repaint(&slot);
            }
            Ok(false) => mouse_down(e),
            Err(err) => notice.set(Some(err.to_string())),
        }
    }
}

/// The pointer over the pane: the block whose gutter it is over, if any, is hovered.
pub fn gutter_hovered(
    slot: Rc<RefCell<Slot>>,
    signals: Option<HostSignals>,
    mut hovered: Signal<Option<BlockId>>,
) -> impl FnMut(PointerEvent) + 'static {
    move |e: PointerEvent| {
        let at = device_at(&e, signals);
        let over = slot
            .borrow()
            .session
            .as_ref()
            .and_then(|s| s.gutter_block(at));
        if over != hovered() {
            hovered.set(over);
        }
    }
}

/// The block bar's copy of a command.
pub fn bar_copy_command(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.copy_command(id);
        }
        ui += 1;
    }
}

/// The block bar's copy of an output.
pub fn bar_copy_output(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.copy_output(id);
        }
        ui += 1;
    }
}

/// The block bar's save of an output to Downloads.
pub fn bar_save(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
    mut notice: Signal<Option<String>>,
) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        let saved = slot
            .borrow_mut()
            .session
            .as_mut()
            .map(|s| s.save_output(id));
        if let Some(Err(err)) = saved {
            notice.set(Some(err.to_string()));
        }
        ui += 1;
    }
}

/// The block bar's re-run: the command is typed, never run.
pub fn bar_rerun(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.rerun(id);
        }
        ui += 1;
        repaint(&slot);
    }
}

/// The block bar's done: the selection goes.
pub fn bar_close(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) -> impl FnMut(()) + 'static {
    move |_: ()| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.clear_block_selection();
        }
        ui += 1;
    }
}

/// The paste confirmation's paste.
pub fn paste_confirm(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) -> impl FnMut(()) + 'static {
    move |_: ()| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.confirm_paste();
        }
        ui += 1;
        repaint(&slot);
    }
}

/// The paste confirmation's cancel.
pub fn paste_cancel(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) -> impl FnMut(()) + 'static {
    move |_: ()| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.cancel_paste();
        }
        ui += 1;
    }
}

/// The ended shell's restart. The new shell starts a frame after the press: the button the
/// person pressed (and the banner it sits in) must not leave the page in the middle of the
/// press, with the pointer still over it.
pub fn restart_pane<T: 'static>(desk: Desk, pane: PaneId) -> impl FnMut(T) + 'static {
    move |_: T| {
        let desk = desk.clone();
        spawn(async move {
            sleep(FRAME_SLACK).await;
            desk.restart(pane);
        });
    }
}

//! The window's event handlers, as functions that return the closure a component hands to its
//! element or control. Each takes the session's slot and the window's signals, and does the
//! session call, then asks the window to draw again.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::base::time::FRAME_SLACK;
use ds::base::time::clock::sleep;
use ds::host::measure::RectProbe;
use ds::host::signals::HostSignals;
use ds::prelude::{Point, Px};
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_pane::keys::Action;

use super::block_menu::BlockMenuAt;
use super::find_bar::StepDirection;
use super::ime::ImeGlue;
use super::slot::{Slot, repaint};
use super::window_help::{device_at, run_query_later};
use crate::hub::Desk;
use crate::session::{HistoryStep, Session};

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
        run_query_later(Rc::clone(&slot), started, ui);
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

/// A press in the pane: a right press on a block picks it for its menu (which opens on the
/// `contextmenu` that follows the release, see [`block_menu_opened`]); anything else is the
/// grid's.
pub fn grid_pressed(
    slot: Rc<RefCell<Slot>>,
    (rect, signals): (RectProbe, Option<HostSignals>),
    mut ui: Signal<u32>,
    mut notice: Signal<Option<String>>,
    mouse_down: Rc<dyn Fn(PointerEvent)>,
) -> impl FnMut(PointerEvent) + 'static {
    move |e: PointerEvent| {
        if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Secondary) {
            let at = device_at(e.client_coordinates(), &rect, signals);
            let block = match slot.borrow_mut().session.as_mut() {
                Some(s) => s.block_menu_press(at),
                None => Ok(None),
            };
            match block {
                Ok(Some(_)) => {
                    ui += 1;
                    repaint(&slot);
                    return;
                }
                Ok(None) => {}
                Err(err) => notice.set(Some(err.to_string())),
            }
        }
        mouse_down(e);
    }
}

/// A right-click's `contextmenu` on the grid: over a block (picked by the press before it), the
/// block menu opens at the pointer. Opening on the release's event, not the press, keeps the
/// release from landing outside a menu that has just opened, and closing it.
pub fn block_menu_opened(
    slot: Rc<RefCell<Slot>>,
    (rect, signals): (RectProbe, Option<HostSignals>),
    mut ui: Signal<u32>,
    mut menu: Signal<Option<BlockMenuAt>>,
) -> impl FnMut(MouseEvent) + 'static {
    move |e: MouseEvent| {
        let at = device_at(e.client_coordinates(), &rect, signals);
        let block = slot
            .borrow_mut()
            .session
            .as_mut()
            .and_then(|s| s.block_menu_press(at).ok().flatten());
        if block.is_some() {
            let c = e.client_coordinates();
            menu.set(Some(BlockMenuAt {
                point: Point {
                    x: Px(c.x as f32),
                    y: Px(c.y as f32),
                },
            }));
            ui += 1;
            repaint(&slot);
        }
    }
}

/// The pointer over the pane: the session keeps where it is, for the link under it.
pub fn pointer_over(
    slot: Rc<RefCell<Slot>>,
    (rect, signals): (RectProbe, Option<HostSignals>),
) -> impl FnMut(PointerEvent) + 'static {
    move |e: PointerEvent| {
        let at = device_at(e.client_coordinates(), &rect, signals);
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.pointer_moved(at);
        }
    }
}

/// The pointer left the pane: no link is under it.
pub fn pointer_gone(slot: Rc<RefCell<Slot>>) -> impl FnMut(PointerEvent) + 'static {
    move |_: PointerEvent| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            s.pointer_left();
        }
    }
}

/// Whether a session call changes what the grid shows. A copy does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Redraw {
    /// The window is drawn again; the grid is as it was.
    WindowOnly,
    /// The grid is painted again too.
    WithGrid,
}

/// The handler for a press that calls `call` on the pane's session with the press's value, then
/// asks for the window (and, with `redraw`, the grid) to be drawn again.
fn on_session<T: 'static>(
    slot: Rc<RefCell<Slot>>,
    mut ui: Signal<u32>,
    redraw: Redraw,
    call: impl Fn(&mut Session, T) + 'static,
) -> impl FnMut(T) + 'static {
    move |value: T| {
        if let Some(s) = slot.borrow_mut().session.as_mut() {
            call(s, value);
        }
        ui += 1;
        if redraw == Redraw::WithGrid {
            repaint(&slot);
        }
    }
}

/// The block toolbar's copy of a command.
pub fn bar_copy_command(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(BlockId) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, id| s.copy_command(id))
}

/// The block toolbar's copy of an output.
pub fn bar_copy_output(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(BlockId) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, id| s.copy_output(id))
}

/// A card's chevron in the block list: folds its output away or brings it back.
pub fn list_fold(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(BlockId) + 'static {
    on_session(slot, ui, Redraw::WithGrid, |s, id| s.toggle_fold_of(id))
}

/// The composer's text as the person edits it.
pub fn composer_input(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(String) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, text| {
        s.set_composer_text(text)
    })
}

/// The composer's Enter: its text goes to the shell.
pub fn composer_submit(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WithGrid, |s, ()| s.composer_submit())
}

/// The composer's arrows through the pane's commands.
pub fn composer_history(
    slot: Rc<RefCell<Slot>>,
    ui: Signal<u32>,
) -> impl FnMut(HistoryStep) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, step| {
        s.composer_history(step)
    })
}

/// The composer's Tab: the shell is asked to complete the line.
pub fn composer_complete(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, ()| s.composer_complete())
}

/// The pick among the words the shell offered moves.
pub fn composer_pick(
    slot: Rc<RefCell<Slot>>,
    ui: Signal<u32>,
) -> impl FnMut(HistoryStep) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, step| {
        s.composer_pick(step)
    })
}

/// A word the shell offered goes into the line: the picked one, or the one pressed.
pub fn composer_take(
    slot: Rc<RefCell<Slot>>,
    ui: Signal<u32>,
) -> impl FnMut(Option<usize>) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, index| {
        s.composer_take_choice(index)
    })
}

/// The words the shell offered are put away.
pub fn composer_close(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, ()| {
        s.composer_close_choices()
    })
}

/// The block toolbar's save of an output to Downloads.
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

/// The block toolbar's re-run: the command is typed, never run.
pub fn bar_rerun(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(BlockId) + 'static {
    on_session(slot, ui, Redraw::WithGrid, |s, id| s.rerun(id))
}

/// The running card's Stop: the program is interrupted.
pub fn block_stop(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WithGrid, |s, ()| s.send_bytes(b"\x03"))
}

/// A header or toolbar press, then the keyboard back to the terminal: the button the person
/// pressed took it, and the terminal is where typing goes.
pub fn then_regain<T: 'static>(
    mut act: impl FnMut(T) + 'static,
    ime: ImeGlue,
    regain: EventHandler<()>,
) -> impl FnMut(T) + 'static {
    move |value: T| {
        act(value);
        ime.take_focus(regain);
    }
}

/// The paste confirmation's paste.
pub fn paste_confirm(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WithGrid, |s, ()| s.confirm_paste())
}

/// The paste confirmation's cancel.
pub fn paste_cancel(slot: Rc<RefCell<Slot>>, ui: Signal<u32>) -> impl FnMut(()) + 'static {
    on_session(slot, ui, Redraw::WindowOnly, |s, ()| s.cancel_paste())
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

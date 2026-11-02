//! The window's event handlers, as functions that return the closure a component hands to its
//! element or control. Each sends its session call through the pane's [`PaneLink`], which carries
//! out what the call left and draws the window again.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::base::time::FRAME_SLACK;
use ds::base::time::clock::sleep;
use ds::host::measure::RectProbe;
use ds::host::signals::HostSignals;
use ds::prelude::{Point, Px};
use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_panes::pane::keys::Action;

use super::block_menu::BlockMenuAt;
use super::find_bar::StepDirection;
use super::ime::ImeGlue;
use super::pane_link::{PaneLink, Redraw};
use super::window_help::{device_at, run_query_later};
use temor_hub::Desk;
use temor_pane_view::session::{HistoryStep, Session};
use temor_ui_base::slot::Slot;

/// The find bar's query, run once typing pauses.
pub fn find_query(link: PaneLink, started: Instant) -> impl FnMut(String) + 'static {
    move |text: String| {
        link.send(Redraw::WindowOnly, |s| {
            s.queue_query(&text, started.elapsed())
        });
        // The search runs once typing pauses (see `run_query_later`).
        run_query_later(Rc::clone(&link.slot), started, link.desk.ui);
    }
}

/// The find bar's next or previous match.
pub fn find_step(link: PaneLink) -> impl FnMut(StepDirection) + 'static {
    move |direction: StepDirection| {
        let action = match direction {
            StepDirection::Forward => Action::FindNext,
            StepDirection::Backward => Action::FindPrevious,
        };
        link.send(Redraw::WithGrid, |s| s.run(action));
    }
}

/// The find bar's close. Its field leaves the page with it, so the keyboard goes back to the
/// terminal region (`regain`, the same focus the region takes on a click).
pub fn find_close(
    link: PaneLink,
    ime: ImeGlue,
    regain: EventHandler<()>,
) -> impl FnMut(()) + 'static {
    move |_: ()| {
        link.send(Redraw::WithGrid, |s| s.run(Action::CloseFind));
        ime.take_focus(regain);
    }
}

/// A press in the pane: a right press on a block picks it for its menu (which opens on the
/// `contextmenu` that follows the release, see [`block_menu_opened`]); anything else is the
/// grid's.
pub fn grid_pressed(
    link: PaneLink,
    (rect, signals): (RectProbe, Option<HostSignals>),
    mouse_down: Rc<dyn Fn(PointerEvent)>,
) -> impl FnMut(PointerEvent) + 'static {
    move |e: PointerEvent| {
        if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Secondary) {
            let at = device_at(e.client_coordinates(), &rect, signals);
            let (block, left) = link.said(|s| s.block_menu_press(at));
            link.carry_out(left);
            match block {
                Some(Ok(Some(_))) => {
                    link.redraw(Redraw::WithGrid);
                    return;
                }
                Some(Err(err)) => link.desk.say(err.to_string()),
                Some(Ok(None)) | None => {}
            }
        }
        mouse_down(e);
    }
}

/// A right-click's `contextmenu` on the grid: over a block (picked by the press before it), the
/// block menu opens at the pointer. Opening on the release's event, not the press, keeps the
/// release from landing outside a menu that has just opened, and closing it.
pub fn block_menu_opened(
    link: PaneLink,
    (rect, signals): (RectProbe, Option<HostSignals>),
    mut menu: Signal<Option<BlockMenuAt>>,
) -> impl FnMut(MouseEvent) + 'static {
    move |e: MouseEvent| {
        let at = device_at(e.client_coordinates(), &rect, signals);
        let (block, left) = link.said(|s| s.block_menu_press(at));
        link.carry_out(left);
        if let Some(Ok(Some(_))) = block {
            let c = e.client_coordinates();
            menu.set(Some(BlockMenuAt {
                point: Point {
                    x: Px(c.x as f32),
                    y: Px(c.y as f32),
                },
            }));
            link.redraw(Redraw::WithGrid);
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

/// The handler for a press that sends `call` with the press's value to the pane's session.
fn on_session<T: 'static>(
    link: PaneLink,
    redraw: Redraw,
    call: impl Fn(&mut Session, T) + 'static,
) -> impl FnMut(T) + 'static {
    move |value: T| {
        link.send(redraw, |s| call(s, value));
    }
}

/// A card's chevron in the block list: folds its output away or brings it back.
pub fn list_fold(link: PaneLink) -> impl FnMut(BlockId) + 'static {
    on_session(link, Redraw::WithGrid, |s, id| s.toggle_fold_of(id))
}

/// A kept command's Put in Prompt in the block list.
pub fn list_put(link: PaneLink) -> impl FnMut(u32) + 'static {
    on_session(link, Redraw::WithGrid, |s, n| s.put_kept(n))
}

/// The composer's text as the person edits it.
pub fn composer_input(link: PaneLink) -> impl FnMut(String) + 'static {
    on_session(link, Redraw::WindowOnly, |s, text| {
        s.set_composer_text(text)
    })
}

/// The composer's Enter: its text goes to the shell.
pub fn composer_submit(link: PaneLink) -> impl FnMut(()) + 'static {
    on_session(link, Redraw::WithGrid, |s, ()| s.composer_submit())
}

/// The composer's arrows through the pane's commands.
pub fn composer_history(link: PaneLink) -> impl FnMut(HistoryStep) + 'static {
    on_session(link, Redraw::WindowOnly, |s, step| s.composer_history(step))
}

/// The composer's Tab: the shell is asked to complete the line.
pub fn composer_complete(link: PaneLink) -> impl FnMut(()) + 'static {
    on_session(link, Redraw::WindowOnly, |s, ()| s.composer_complete())
}

/// The pick among the words the shell offered moves.
pub fn composer_pick(link: PaneLink) -> impl FnMut(HistoryStep) + 'static {
    on_session(link, Redraw::WindowOnly, |s, step| s.composer_pick(step))
}

/// A word the shell offered goes into the line: the picked one, or the one pressed.
pub fn composer_take(link: PaneLink) -> impl FnMut(Option<usize>) + 'static {
    on_session(link, Redraw::WindowOnly, |s, index| {
        s.composer_take_choice(index)
    })
}

/// The words the shell offered are put away.
pub fn composer_close(link: PaneLink) -> impl FnMut(()) + 'static {
    on_session(link, Redraw::WindowOnly, |s, ()| s.composer_close_choices())
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
pub fn paste_confirm(link: PaneLink) -> impl FnMut(()) + 'static {
    on_session(link, Redraw::WithGrid, |s, ()| s.confirm_paste())
}

/// The paste confirmation's cancel.
pub fn paste_cancel(link: PaneLink) -> impl FnMut(()) + 'static {
    on_session(link, Redraw::WindowOnly, |s, ()| s.cancel_paste())
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

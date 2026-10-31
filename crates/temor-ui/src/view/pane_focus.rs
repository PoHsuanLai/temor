//! Which pane has the keyboard (PLAN §6): the region's focus events, the pane that becomes the
//! keyboard pane taking the keyboard on the page, and the region's element hearing the input
//! method. The hooks run in the order the pane's component calls this.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use ds::edit::input::EditInput;
use ds::edit::pointer::EditFocus;
use ds::host::ime::ImeEvent;
use ds::host::measure::RectProbe;
use temor_client::model::pane::PaneFocus;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::workspace::WorkspaceIn;

use super::ime::{ImeGlue, focus_handler};
use super::slot::Slot;
use super::window_help::{focus_in, set_focus};
use crate::hub::Desk;

/// What the pane's region is given for the keyboard.
pub struct FocusWiring {
    /// Where the keyboard goes back to when the find bar closes (its field leaves the page).
    pub regain: EventHandler<()>,
    /// The region gained or lost the keyboard.
    pub on_focus: EventHandler<EditFocus>,
    /// The region's element mounted: the rect and the input method hear it.
    pub on_mounted: EventHandler<MountedEvent>,
}

impl std::fmt::Debug for FocusWiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusWiring").finish_non_exhaustive()
    }
}

/// Wires the keyboard for the pane `pane`. `heard` takes the input method's events.
pub fn use_pane_focus(
    pane: PaneId,
    desk: &Desk,
    slot: &Rc<RefCell<Slot>>,
    ime: &ImeGlue,
    rect: RectProbe,
    apply: impl Fn(EditInput) + Clone + 'static,
    heard: EventHandler<ImeEvent>,
) -> FocusWiring {
    // Whether this pane had the keyboard the last time it was drawn.
    let had_keyboard = use_hook(|| Rc::new(Cell::new(false)));

    // The region has the keyboard: the workspace and the session know, and the input method is on
    // and placed.
    let gained = {
        let (ime, slot, desk) = (ime.clone(), Rc::clone(slot), desk.clone());
        move || {
            set_focus(&slot, PaneFocus::Focused);
            desk.pane_focused(pane);
            if desk.hub.keyboard_pane() != Some(pane) {
                desk.act(WorkspaceIn::Focus(pane));
            }
            focus_in(ime.clone(), Rc::clone(&slot), rect);
        }
    };
    let on_focus = {
        let mut inner = focus_handler(Rc::clone(slot), ime.clone(), apply, gained.clone());
        let had = Rc::clone(&had_keyboard);
        let desk = desk.clone();
        move |focus: EditFocus| {
            // The keyboard left the page region (the sidebar took it, say): the pane takes it
            // back the next time the window is told it is the keyboard pane.
            if matches!(focus, EditFocus::Out) {
                had.set(false);
                desk.pane_blurred(pane);
            }
            inner(focus);
        }
    };
    let regain = EventHandler::new({
        let gained = gained.clone();
        move |()| gained()
    });

    // The pane that becomes the keyboard pane (a chord moved focus, a tile was pressed) takes the
    // keyboard on the page too; not while the bar's field or the composer's has it, and back when
    // the bar closes, the composer goes (a command runs) or a menu's pick gives it back.
    {
        let (ime, gained, desk, had, slot) = (
            ime.clone(),
            gained.clone(),
            desk.clone(),
            Rc::clone(&had_keyboard),
            Rc::clone(slot),
        );
        let regained = use_hook(|| Rc::new(Cell::new(0)));
        use_effect(move || {
            let _ = (desk.ui)();
            let regain = (desk.regain)();
            let asked = regained.replace(regain) != regain;
            // Not while the bar's or the find bar's field has it (Escape in the find hands it back).
            let finding = find_is_open(&slot);
            let has = desk.hub.keyboard_pane() == Some(pane)
                && !desk.hub.bar_is_open()
                && !finding
                && !composing(&slot);
            let before = had.replace(has);
            if has && (!before || asked) {
                let gained = gained.clone();
                ime.take_focus(EventHandler::new(move |()| gained()));
            }
        });
    }

    // The region's element: rect and input method hear it. The first layout gives the pane that
    // has the keyboard the keyboard, as a person's click would.
    let on_mounted = {
        let (ime, gained, desk) = (ime.clone(), gained, desk.clone());
        move |event: MountedEvent| {
            rect.on_mounted(event.clone());
            let element = event.data();
            ime.element.replace(Some(Rc::clone(&element)));
            let (ime, gained) = (ime.clone(), gained.clone());
            let keyboard = desk.hub.keyboard_pane() == Some(pane);
            let focus = ime.clone();
            ime.listen(element, heard, move || {
                if keyboard {
                    focus.take_focus(EventHandler::new(move |()| gained()));
                }
            });
        }
    };
    FocusWiring {
        regain,
        on_focus: EventHandler::new(on_focus),
        on_mounted: EventHandler::new(on_mounted),
    }
}

/// Whether the list shows the composer: its field has the keyboard.
fn composing(slot: &Rc<RefCell<Slot>>) -> bool {
    slot.borrow()
        .session
        .as_ref()
        .is_some_and(|s| s.composer_shown())
}

/// Whether the pane's find bar is up: its field has the keyboard until it closes.
fn find_is_open(slot: &Rc<RefCell<Slot>>) -> bool {
    slot.borrow()
        .session
        .as_ref()
        .is_some_and(|s| s.finder().is_some())
}

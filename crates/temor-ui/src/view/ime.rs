//! The input method's glue: the IME listener on the terminal region, the cursor area it places
//! its candidates at, the composition state the keys wait for, and the keyboard focus. The
//! composition machine and the focus write are quire's (`ds::edit::composition`,
//! `ds::focus::soon`), and so is the retry of the IME host seams while the document is busy
//! (`write_soon_then`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use ds::components::editor::write_soon::{write_soon, write_soon_then};
use ds::edit::composition::{Composing, on_ime, settle};
use ds::edit::input::{Composition, EditInput};
use ds::focus::select::Select;
use ds::focus::soon::focus_soon_told;
use ds::host::document::DocumentHost;
use ds::host::ime::{ImeEvent, ImeListener, ImeSwitch};
use ds::prelude::Rect;

use super::slot::Slot;
use super::window_help::{send_text, set_focus};
use ds::edit::pointer::EditFocus;
use temor_core::notify::Focus;

/// The region's IME state, shared by its handlers.
#[derive(Clone)]
pub struct ImeGlue {
    pub host: Rc<dyn DocumentHost>,
    pub composing: Rc<Cell<Composing>>,
    pub element: Rc<RefCell<Option<Rc<MountedData>>>>,
    pub listener: Rc<RefCell<Option<ImeListener>>>,
}

impl ImeGlue {
    pub fn new(host: Rc<dyn DocumentHost>) -> Self {
        Self {
            host,
            composing: Rc::new(Cell::new(Composing::Idle)),
            element: Rc::new(RefCell::new(None)),
            listener: Rc::new(RefCell::new(None)),
        }
    }

    /// The region left the page: the handles to its element are dropped, so nothing asks the host
    /// about an element that is gone.
    pub fn release(&self) {
        self.element.replace(None);
        self.listener.replace(None);
    }

    /// One IME event: the composition steps it makes, handed to `apply`.
    pub fn event(&self, event: ImeEvent, apply: &dyn Fn(EditInput)) {
        let (next, inputs) = on_ime(self.composing.get(), event);
        self.composing.set(next);
        inputs.into_iter().for_each(apply);
    }

    /// Ends a composition the window is leaving.
    pub fn settle(&self, apply: &dyn Fn(EditInput)) {
        let (idle, ended) = settle(self.composing.get());
        self.composing.set(idle);
        ended.into_iter().for_each(apply);
    }

    /// Turns the input method on or off for the region, a frame later while the document is
    /// busy. `then` runs once the host has answered (or given up).
    pub fn switch(&self, to: ImeSwitch, then: impl FnOnce() + 'static) {
        let Some(el) = self.element.borrow().clone() else {
            return then();
        };
        write_soon_then(
            Rc::clone(&self.host),
            el,
            move |edit, el| edit.ime().switch(el, to),
            move |_| then(),
        );
    }

    /// Places the input method's candidates at `area` (window logical pixels).
    pub fn cursor_area(&self, area: Rect) {
        let Some(el) = self.element.borrow().clone() else {
            return;
        };
        write_soon(Rc::clone(&self.host), el, move |edit, el| {
            edit.ime().cursor_area(el, area)
        });
    }

    /// Gives the region the keyboard, a frame later while the document is busy (quire's
    /// `focus_soon_told`). `told` is called once the focus has moved: a focus the app asks for
    /// sends no focus event to the region's own handler.
    pub fn take_focus(&self, told: EventHandler<()>) {
        if let Some(el) = self.element.borrow().clone() {
            focus_soon_told(el, Select::None, told);
        }
    }

    /// Registers the region for IME events. `then` runs once the host has answered (or given
    /// up).
    pub fn listen(
        &self,
        element: Rc<MountedData>,
        heard: EventHandler<ImeEvent>,
        then: impl FnOnce() + 'static,
    ) {
        let slot = Rc::clone(&self.listener);
        write_soon_then(
            Rc::clone(&self.host),
            element,
            move |edit, el| edit.ime().listen(el, heard),
            move |listener| {
                *slot.borrow_mut() = listener;
                then();
            },
        );
    }
}
/// The region's focus: the keyboard in (`gained`), or out, when the composition ends and the
/// input method turns off.
pub fn focus_handler(
    slot: Rc<RefCell<Slot>>,
    ime: ImeGlue,
    apply: impl Fn(EditInput) + Clone + 'static,
    gained: impl Fn() + Clone + 'static,
) -> impl FnMut(EditFocus) + 'static {
    move |focus: EditFocus| match focus {
        EditFocus::In => gained(),
        EditFocus::Out => {
            set_focus(&slot, Focus::Unfocused);
            ime.settle(&apply);
            ime.switch(ImeSwitch::Off, || {});
        }
    }
}

impl std::fmt::Debug for ImeGlue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImeGlue")
            .field("composing", &self.composing.get())
            .finish_non_exhaustive()
    }
}

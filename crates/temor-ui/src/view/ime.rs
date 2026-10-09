//! The input method's glue: the IME listener on the terminal region, the cursor area it places
//! its candidates at, the composition state the keys wait for, and the keyboard focus. The
//! composition machine and the focus write are quire's (`ds::edit::composition`,
//! `ds::focus::soon`); the IME host seams are retried here while the document is busy, since
//! quire's `write_soon` is crate-private at v0.3.1 (an ask in the report).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::time::FRAME_SLACK;
use ds::base::time::clock::sleep;
use ds::edit::composition::{Composing, on_ime, settle};
use ds::edit::input::{Composition, EditInput};
use ds::focus::select::Select;
use ds::focus::soon::focus_soon_told;
use ds::host::document::DocumentHost;
use ds::host::ime::{ImeEvent, ImeListener, ImeSwitch};
use ds::host::parts::EditHost;
use ds::host::probe::Probe;
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

    /// Turns the input method on or off for the region.
    pub async fn switch(&self, to: ImeSwitch) -> bool {
        let Some(el) = self.element.borrow().clone() else {
            return false;
        };
        with_edit(&self.host, |e| e.ime().switch(&el, to))
            .await
            .is_some()
    }

    /// Places the input method's candidates at `area` (window logical pixels).
    pub async fn cursor_area(&self, area: Rect) {
        let Some(el) = self.element.borrow().clone() else {
            return;
        };
        let _ = with_edit(&self.host, |e| e.ime().cursor_area(&el, area)).await;
    }

    /// Gives the region the keyboard, a frame later while the document is busy (quire's
    /// `focus_soon_told`). `told` is called once the focus has moved: a focus the app asks for
    /// sends no focus event to the region's own handler.
    pub fn take_focus(&self, told: EventHandler<()>) {
        if let Some(el) = self.element.borrow().clone() {
            focus_soon_told(el, Select::None, told);
        }
    }

    /// Registers the region for IME events.
    pub async fn listen(&self, element: Rc<MountedData>, heard: EventHandler<ImeEvent>) {
        let listener = with_edit(&self.host, |e| e.ime().listen(&element, heard)).await;
        *self.listener.borrow_mut() = listener;
    }
}

/// Where the input method's composed text goes: the program, and the preedit line the window
/// draws while a composition is under way.
pub fn composed_sink(
    slot: Rc<RefCell<Slot>>,
    preedit: Signal<Option<String>>,
) -> impl Fn(EditInput) + Clone + 'static {
    move |input: EditInput| {
        let mut preedit = preedit;
        match input {
            EditInput::Text(text) => send_text(&slot, &text),
            EditInput::Composition(Composition::Update { text, .. }) => {
                preedit.set((!text.is_empty()).then_some(text));
            }
            EditInput::Composition(Composition::End { text }) => {
                preedit.set(None);
                send_text(&slot, &text);
            }
            _ => {}
        }
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
            let ime = ime.clone();
            spawn(async move {
                ime.switch(ImeSwitch::Off).await;
            });
        }
    }
}

/// A host write, retried while the document is busy (quire's `write_soon` is crate-private).
pub async fn with_edit<T>(
    host: &Rc<dyn DocumentHost>,
    f: impl Fn(&dyn EditHost) -> Probe<T>,
) -> Option<T> {
    for _ in 0..16 {
        let edit = host.edit()?;
        match f(edit) {
            Probe::Found(v) => return Some(v),
            Probe::Busy => sleep(FRAME_SLACK).await,
            Probe::Unknown => return None,
        }
    }
    None
}

impl std::fmt::Debug for ImeGlue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImeGlue")
            .field("composing", &self.composing.get())
            .finish_non_exhaustive()
    }
}

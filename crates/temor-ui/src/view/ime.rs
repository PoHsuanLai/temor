//! The input method's glue: the IME listener on the terminal region, the cursor area it places
//! its candidates at, and the composition steps the keys wait for. It follows quire's
//! `EditSurface` through the host seams (`DocumentHost::edit()`), retrying while the document is
//! busy. quire's own helpers are crate-private at v0.2.30 (asks in the report).
// TODO(quire 0.2.31): focus_soon_told and the public composition machine replace these copies.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::time::FRAME_SLACK;
use ds::base::time::clock::sleep;
use ds::edit::input::EditInput;
use ds::host::document::DocumentHost;
use ds::host::focused::Focused;
use ds::host::ime::{ImeEvent, ImeListener, ImeSwitch};
use ds::host::parts::EditHost;
use ds::host::probe::Probe;
use ds::prelude::Rect;

use crate::input::{Composing, on_ime, settle};

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

    /// Gives the region the keyboard through the host, retrying while the document is busy.
    /// Blitz sends no focus event for this, so the caller turns the input method on itself.
    pub async fn take_focus(&self) -> bool {
        let Some(el) = self.element.borrow().clone() else {
            return false;
        };
        for _ in 0..16 {
            match self.host.focus().focus(&el) {
                Focused::Done => return true,
                Focused::Busy => sleep(FRAME_SLACK).await,
                Focused::Unknown => return false,
            }
        }
        false
    }

    /// Registers the region for IME events.
    pub async fn listen(&self, element: Rc<MountedData>, heard: EventHandler<ImeEvent>) {
        let listener = with_edit(&self.host, |e| e.ime().listen(&element, heard)).await;
        *self.listener.borrow_mut() = listener;
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

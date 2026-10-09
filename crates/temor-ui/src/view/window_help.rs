//! Free helpers of the window: pixels, the input method's cursor area, the find query's timer and
//! the focus and pointer glue. They take the session's slot and keep no state of their own.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use ds::host::ime::ImeSwitch;
use ds::host::measure::RectProbe;
use ds::host::signals::HostSignals;
use ds::prelude::*;
use temor_core::notify::Focus;
use temor_render::{PixelSize, Scale};

use super::convert::{device_point, scale_of};
use super::ime::ImeGlue;
use super::slot::{Slot, repaint};
use crate::session::QUERY_DEBOUNCE;

/// The pane's size in device pixels, from its logical size and scale.
pub(super) fn device_pixels(width: f32, height: f32, scale: Scale) -> PixelSize {
    let f = f64::from(scale.factor());
    PixelSize::new(
        (f64::from(width) * f).floor().max(0.0) as u32,
        (f64::from(height) * f).floor().max(0.0) as u32,
    )
}

/// Sends composed text to the program, if there is a session.
pub(super) fn send_text(slot: &RefCell<Slot>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(s) = slot.borrow().session.as_ref() {
        s.pane().send(text.as_bytes());
    }
}

/// The pane has the keyboard: the input method is on and placed at the cursor.
pub(super) fn focus_in(ime: ImeGlue, slot: Rc<RefCell<Slot>>, rect: RectProbe) {
    spawn(async move {
        ime.switch(ImeSwitch::On).await;
        ime.cursor_area(cursor_rect(&slot, rect)).await;
    });
}

/// Puts the input method's candidates at the cursor after the screen changed.
pub(super) fn place_cursor(ime: ImeGlue, slot: Rc<RefCell<Slot>>, rect: RectProbe) {
    spawn(async move {
        ime.cursor_area(cursor_rect(&slot, rect)).await;
    });
}

/// The cursor's cell in window logical pixels (zero until the pane is laid out and shown).
pub(super) fn cursor_rect(slot: &RefCell<Slot>, rect: RectProbe) -> Rect {
    let origin = rect
        .rect()
        .map_or((0.0, 0.0), |r| (r.origin.x.0, r.origin.y.0));
    let cell = slot
        .borrow()
        .session
        .as_ref()
        .and_then(|s| s.cursor_logical())
        .unwrap_or([0.0; 4]);
    Rect {
        origin: Point {
            x: Px(origin.0 + cell[0]),
            y: Px(origin.1 + cell[1]),
        },
        size: Size {
            width: Px(cell[2]),
            height: Px(cell[3]),
        },
    }
}

/// The message the session has for the person, if any; taking it clears it.
pub(super) fn take_notice(slot: &RefCell<Slot>) -> Option<String> {
    slot.borrow_mut()
        .session
        .as_mut()
        .and_then(|s| s.take_notice())
}

/// Runs the find query once the person has paused typing for [`QUERY_DEBOUNCE`].
pub(super) fn run_query_later(slot: Rc<RefCell<Slot>>, started: Instant) {
    spawn(async move {
        sleep(QUERY_DEBOUNCE).await;
        let ran = slot
            .borrow_mut()
            .session
            .as_mut()
            .map(|s| s.run_due_query(started.elapsed()));
        if matches!(ran, Some(Ok(true))) {
            repaint(&slot);
        }
    });
}

/// A pointer event's place in the pane, in device pixels.
pub(super) fn device_at(
    event: &PointerEvent,
    signals: Option<HostSignals>,
) -> temor_vt::SurfacePoint {
    let p = event.element_coordinates();
    device_point(p.x, p.y, scale_of(signals.as_ref()))
}

/// Tells the session whether the pane has the keyboard (a notice goes out only when it does not),
/// and draws the cursor as the pane's focus says.
pub(super) fn set_focus(slot: &RefCell<Slot>, focus: Focus) {
    if let Some(s) = slot.borrow_mut().session.as_mut() {
        s.set_focus(focus);
    }
    repaint(slot);
}

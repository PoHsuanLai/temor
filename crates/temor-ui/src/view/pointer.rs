//! The terminal's pointer (PLAN §6): the press, release and wheel go to the session, and a drag
//! that leaves the pane is followed by pointer capture. Called from the window's component, so
//! its hooks run in the component's order.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::measure::RectProbe;
use ds::host::pointer_capture::{PointerCapture, PointerHold, use_pointer_capture};
use ds::host::signals::HostSignals;
use temor_vt::MouseButton as VtButton;

use super::convert::{device_point, scale_of, vt_button, vt_mods, wheel_lines};
use super::slot::{Slot, repaint};

/// The pane's pointer handlers: the capture for drags, and the press, release and wheel.
#[derive(Clone)]
pub struct PaneMouse {
    pub capture: PointerCapture,
    pub down: Rc<dyn Fn(PointerEvent)>,
    pub up: Rc<dyn Fn(PointerEvent)>,
    pub wheel: Rc<dyn Fn(WheelEvent)>,
}

impl std::fmt::Debug for PaneMouse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneMouse").finish_non_exhaustive()
    }
}

/// Which path reports a press's release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseOwner {
    /// The host's pointer capture, which follows a primary press.
    Host,
    /// The element's own release handler.
    Element,
}

/// Builds the pane's pointer handlers for this component.
pub fn use_pane_mouse(
    slot: Rc<RefCell<Slot>>,
    rect: RectProbe,
    signals: Option<HostSignals>,
    notice: Signal<Option<String>>,
    started: Instant,
) -> PaneMouse {
    // Who reports the release of the press in progress. The host's capture only follows a primary
    // press, so a primary press it captures is released through the capture, and every other press
    // is released through the element (PLAN §6). Exactly one of them reports.
    let owner = Rc::new(Cell::new(ReleaseOwner::Element));
    let capture = {
        let (slot, signals, mut notice, owner) =
            (Rc::clone(&slot), signals, notice, Rc::clone(&owner));
        use_pointer_capture(move |captured: CapturedPointer| {
            let scale = scale_of(signals.as_ref());
            let origin = rect
                .rect()
                .map_or((0.0, 0.0), |r| (r.origin.x.0, r.origin.y.0));
            let point = device_point(
                f64::from(captured.at.x.0 - origin.0),
                f64::from(captured.at.y.0 - origin.1),
                scale,
            );
            let mods = vt_mods(captured.modifiers);
            // The host only captures primary presses, so its release is the primary button's.
            let result = slot
                .borrow_mut()
                .session
                .as_mut()
                .map(|s| match captured.phase {
                    PointerPhase::Drag => s.drag_to(point),
                    PointerPhase::Release if owner.get() == ReleaseOwner::Host => {
                        s.release(point, VtButton::Left, mods)
                    }
                    PointerPhase::Release | PointerPhase::Press => Ok(()),
                });
            if let Some(Err(err)) = result {
                notice.set(Some(err.to_string()));
            }
            repaint(&slot);
        })
    };

    let down = {
        let (slot, signals, notice, owner) = (Rc::clone(&slot), signals, notice, Rc::clone(&owner));
        move |event: PointerEvent| {
            // Back and forward buttons do not select or report anything here.
            let Some(button) = event.trigger_button().and_then(vt_button) else {
                return;
            };
            owner.set(match button {
                VtButton::Left => match capture.begin() {
                    PointerHold::Captured => ReleaseOwner::Host,
                    PointerHold::Local => ReleaseOwner::Element,
                },
                _ => ReleaseOwner::Element,
            });
            let p = event.element_coordinates();
            let scale = scale_of(signals.as_ref());
            let result = slot.borrow_mut().session.as_mut().map(|s| {
                s.press(
                    device_point(p.x, p.y, scale),
                    button,
                    vt_mods(event.modifiers()),
                    started.elapsed(),
                )
            });
            if let Some(Err(err)) = result {
                let mut notice = notice;
                notice.set(Some(err.to_string()));
            }
            repaint(&slot);
        }
    };

    let up = {
        let (slot, signals, notice, owner) = (Rc::clone(&slot), signals, notice, Rc::clone(&owner));
        move |event: PointerEvent| {
            let Some(button) = event.trigger_button().and_then(vt_button) else {
                return;
            };
            // One source for the release: the host's capture if it owns this press.
            if owner.get() == ReleaseOwner::Host {
                return;
            }
            let p = event.element_coordinates();
            let scale = scale_of(signals.as_ref());
            let mut notice = notice;
            let released = slot.borrow_mut().session.as_mut().map(|s| {
                s.release(
                    device_point(p.x, p.y, scale),
                    button,
                    vt_mods(event.modifiers()),
                )
            });
            if let Some(Err(err)) = released {
                notice.set(Some(err.to_string()));
            }
            repaint(&slot);
        }
    };

    let wheel = {
        let (slot, signals) = (Rc::clone(&slot), signals);
        move |event: WheelEvent| {
            event.prevent_default();
            let p = event.element_coordinates();
            let scale = scale_of(signals.as_ref());
            let (direction, lines) = wheel_lines(event.delta());
            let at = device_point(p.x, p.y, scale);
            if let Some(s) = slot.borrow_mut().session.as_mut() {
                let _ = s.wheel(at, direction, lines, vt_mods(event.modifiers()));
            }
            repaint(&slot);
        }
    };

    PaneMouse {
        capture,
        down: Rc::new(down),
        up: Rc::new(up),
        wheel: Rc::new(wheel),
    }
}

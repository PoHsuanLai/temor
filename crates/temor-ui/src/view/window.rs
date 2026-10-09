//! The window (PLAN §2, temor-ui): one terminal pane with its find bar and paste confirmation.
//! The component wires Dioxus events to the session and the session's state to elements; the
//! session does the work (see `slot` for the state and `convert` for the event values).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::edit::input::{Composition, EditInput};
use ds::host::document::use_document_host;
use ds::host::ime::{ImeEvent, ImeSwitch};
use ds::host::measure::{RectProbe, use_rect};
use ds::host::signals::HostSignals;
use ds::prelude::*;
use ds_blitz::{Sampling, TextureFit, TextureLayer, use_gpu};
use temor_core::terminal::action::Action;
use temor_render::{PixelSize, Scale};
use temor_vt::{Ime, KeyAction};

use ds::base::time::clock::sleep;

use super::confirm::PasteConfirm;
use super::convert::scale_of;
use super::find_bar::{FindBar, StepDirection};
use super::ime::ImeGlue;
use super::launch::Launch;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Slot, layout, repaint};
use crate::input::{Composing, RawKey, Repeat, stroke};
use crate::session::QUERY_DEBOUNCE;

/// The window's root component.
#[allow(non_snake_case)]
pub fn App() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let host = use_hook(use_document_host);
    let gpu = use_gpu();
    let rect = use_rect();
    let signals = use_hook(try_consume_context::<HostSignals>);
    let slot = use_hook(|| Rc::new(RefCell::new(Slot::default())));
    let texture = use_hook(|| gpu.handle());
    let ime = use_hook(|| ImeGlue::new(Rc::clone(&host)));
    let started = use_hook(Instant::now);
    let mut ui = use_signal(|| 0u32);
    let preedit = use_signal(|| None::<String>);
    let mut notice = use_signal(|| None::<String>);

    // Every change of the pane's size, scale or device lays it out again.
    {
        let (slot, texture, launch, signals, gpu) = (
            Rc::clone(&slot),
            texture.clone(),
            launch.clone(),
            signals,
            gpu.clone(),
        );
        use_effect(move || {
            // Also re-run when the window asks for a new shell (after the program ended).
            let _ = ui();
            let scale = scale_of(signals.as_ref());
            let Some(logical) = rect.rect() else { return };
            let pixels = device_pixels(logical.size.width.0, logical.size.height.0, scale);
            let device = match gpu.device().is_some() {
                true => Device::Ready,
                false => Device::NotYet,
            };
            if let Some(text) = layout(&slot, ui, &launch, &texture, pixels, scale, device) {
                notice.set(Some(text));
            }
        });
    }

    // Composed text and the preedit line, from the input method.
    let apply = {
        let slot = Rc::clone(&slot);
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
    };
    let heard = {
        let ime = ime.clone();
        let apply = apply.clone();
        use_callback(move |event: ImeEvent| ime.event(event, &apply))
    };

    let on_key = {
        let slot = Rc::clone(&slot);
        let ime = ime.clone();
        let apply = apply.clone();
        move |event: KeyboardEvent, action: KeyAction| {
            let key = event.key();
            let code = event.code();
            let composing = match ime.composing.get() {
                Composing::Active => Ime::Composing,
                Composing::Idle | Composing::Cleared => Ime::Idle,
            };
            let raw = RawKey {
                key: &key,
                code: &code,
                held: event.modifiers(),
                action,
                repeating: match event.is_auto_repeating() {
                    true => Repeat::Again,
                    false => Repeat::First,
                },
                ime: composing,
            };
            if action != KeyAction::Release {
                event.prevent_default();
                ime.settle(&apply);
            }
            if composing == Ime::Composing {
                return;
            }
            // An unidentified key is ignored; the rest go to the session.
            let outcome = match stroke(&raw) {
                Some(stroke) => slot.borrow_mut().session.as_mut().map(|s| s.key(stroke)),
                None => None,
            };
            if let Some(Err(err)) = outcome {
                notice.set(Some(err.to_string()));
            }
            if let Some(message) = take_notice(&slot) {
                notice.set(Some(message));
            }
            ui += 1;
            repaint(&slot);
            place_cursor(ime.clone(), Rc::clone(&slot), rect);
        }
    };
    let (mut key_down, mut key_up) = (on_key.clone(), on_key);

    let mouse = use_pane_mouse(Rc::clone(&slot), rect, signals, notice, started);

    let on_focus = {
        let ime = ime.clone();
        let slot = Rc::clone(&slot);
        move |_: FocusEvent| focus_in(ime.clone(), Rc::clone(&slot), rect)
    };
    let on_blur = {
        let ime = ime.clone();
        let apply = apply.clone();
        move |_: FocusEvent| {
            ime.settle(&apply);
            let ime = ime.clone();
            spawn(async move {
                ime.switch(ImeSwitch::Off).await;
            });
        }
    };

    let on_mounted = {
        let ime = ime.clone();
        let slot = Rc::clone(&slot);
        move |event: MountedEvent| {
            rect.on_mounted(event.clone());
            let element = event.data();
            ime.element.replace(Some(Rc::clone(&element)));
            let (ime, slot) = (ime.clone(), Rc::clone(&slot));
            spawn(async move {
                ime.listen(element, heard).await;
                // The first layout gives the pane the keyboard, as a person's click would.
                if ime.take_focus().await {
                    focus_in(ime, slot, rect);
                }
            });
        }
    };

    let _ = ui();
    let view = slot.borrow().session.as_ref().map(|s| {
        (
            s.finder().map(|f| (s.find_text().to_owned(), f.position())),
            s.pending_paste().map(|p| p.reason),
            s.ended(),
        )
    });
    let (find, pending, ended) = view.unwrap_or((None, None, false));
    let message = notice();
    // TODO(quire 0.2.31): full-height root (RootExtent::Window) and quire's layout components and
    // spacing tokens replace the inline layout below (CONSUMING.md rules 1 and 2).
    let pane_style =
        "position:relative; flex:1 1 auto; min-height:0; width:100%; outline:none; overflow:hidden";
    rsx! {
        Ds { appearance: Appearance::default(), material: Material::Window, extent: RootExtent::Viewport,
            // TODO(quire 0.2.31): a full-height root (RootExtent::Window) and a layout component for
            // this column; v0.2.30 has neither, so the flex column stays a plain element.
            div { style: "display:flex; flex-direction:column; width:100%; height:100vh",
                if let Some((query, position)) = find {
                    FindBar {
                        query,
                        position,
                        on_query: {
                            let slot = Rc::clone(&slot);
                            move |text: String| {
                                if let Some(s) = slot.borrow_mut().session.as_mut() {
                                    s.queue_query(&text, started.elapsed());
                                }
                                // The search runs once typing pauses (see `run_query_later`).
                                run_query_later(Rc::clone(&slot), started);
                                ui += 1;
                            }
                        },
                        on_step: {
                            let slot = Rc::clone(&slot);
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
                        },
                        on_close: {
                            let slot = Rc::clone(&slot);
                            move |_: ()| {
                                if let Some(s) = slot.borrow_mut().session.as_mut() {
                                    let _ = s.run(Action::CloseFind);
                                }
                                ui += 1;
                                repaint(&slot);
                            }
                        },
                    }
                }
                div {
                    class: "temor-pane",
                    tabindex: "0",
                    role: "application",
                    "aria-label": "Terminal",
                    "data-keys": "capture",
                    "data-wheel": "capture",
                    style: "{pane_style}",
                    onmounted: move |e| {
                        mouse.capture.on_mounted(e.clone());
                        on_mounted(e);
                    },
                    onkeydown: move |e| key_down(e, KeyAction::Press),
                    onkeyup: move |e| key_up(e, KeyAction::Release),
                    onfocus: on_focus,
                    onblur: on_blur,
                    onpointerdown: move |e| (mouse.down)(e),
                    onpointerup: move |e| (mouse.up)(e),
                    onwheel: move |e| (mouse.wheel)(e),
                    TextureLayer { texture: Some(texture.clone()), fit: TextureFit::Actual, sampling: Sampling::Nearest }
                    if let Some(text) = preedit() {
                        div { style: "position:absolute; left:0; bottom:0; white-space:pre; text-decoration:underline", "{text}" }
                    }
                }
                if let Some(reason) = pending {
                    PasteConfirm {
                        reason,
                        on_paste: {
                            let slot = Rc::clone(&slot);
                            move |_: ()| {
                                if let Some(s) = slot.borrow_mut().session.as_mut() {
                                    s.confirm_paste();
                                }
                                ui += 1;
                                repaint(&slot);
                            }
                        },
                        on_cancel: {
                            let slot = Rc::clone(&slot);
                            move |_: ()| {
                                if let Some(s) = slot.borrow_mut().session.as_mut() {
                                    s.cancel_paste();
                                }
                                ui += 1;
                            }
                        },
                    }
                }
                if ended {
                    InlineBanner {
                        severity: Severity::Warn,
                        text: "The shell has ended.".to_owned(),
                        actions: rsx! {
                            Button {
                                label: "Restart".to_owned(),
                                title: Some("Start a new shell".to_owned()),
                                onclick: {
                                    let slot = Rc::clone(&slot);
                                    move |_| {
                                        // The old session shuts its program down when it is dropped.
                                        let mut s = slot.borrow_mut();
                                        s.session = None;
                                        s.events = None;
                                        drop(s);
                                        ui += 1;
                                    }
                                },
                            }
                        },
                    }
                }
                if let Some(text) = message {
                    InlineBanner {
                        severity: Severity::Warn,
                        text,
                        onclose: move |_| notice.set(None),
                    }
                }
            }
        }
    }
}

/// The pane's size in device pixels, from its logical size and scale.
fn device_pixels(width: f32, height: f32, scale: Scale) -> PixelSize {
    let f = f64::from(scale.factor());
    PixelSize::new(
        (f64::from(width) * f).floor().max(0.0) as u32,
        (f64::from(height) * f).floor().max(0.0) as u32,
    )
}

/// Sends composed text to the program, if there is a session.
fn send_text(slot: &RefCell<Slot>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(s) = slot.borrow().session.as_ref() {
        s.pane().send(text.as_bytes());
    }
}

/// The pane has the keyboard: the input method is on and placed at the cursor.
fn focus_in(ime: ImeGlue, slot: Rc<RefCell<Slot>>, rect: RectProbe) {
    spawn(async move {
        ime.switch(ImeSwitch::On).await;
        ime.cursor_area(cursor_rect(&slot, rect)).await;
    });
}

/// Puts the input method's candidates at the cursor after the screen changed.
fn place_cursor(ime: ImeGlue, slot: Rc<RefCell<Slot>>, rect: RectProbe) {
    spawn(async move {
        ime.cursor_area(cursor_rect(&slot, rect)).await;
    });
}

/// The cursor's cell in window logical pixels (zero until the pane is laid out and shown).
fn cursor_rect(slot: &RefCell<Slot>, rect: RectProbe) -> Rect {
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
fn take_notice(slot: &RefCell<Slot>) -> Option<String> {
    slot.borrow_mut()
        .session
        .as_mut()
        .and_then(|s| s.take_notice())
}

/// Runs the find query once the person has paused typing for [`QUERY_DEBOUNCE`].
fn run_query_later(slot: Rc<RefCell<Slot>>, started: Instant) {
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

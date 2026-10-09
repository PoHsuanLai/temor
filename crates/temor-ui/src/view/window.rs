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
use ds::host::measure::use_rect;
use ds::host::signals::HostSignals;
use ds::prelude::*;
use ds_blitz::{Sampling, TextureFit, TextureLayer, use_gpu};
use temor_vt::{Ime, KeyAction};

use super::blocks::{BlockBar, Chrome, GutterMarks, chrome_of};
use super::confirm::PasteConfirm;
use super::convert::scale_of;
use super::find_bar::FindBar;
use super::handlers::{
    bar_close, bar_copy_command, bar_copy_output, bar_rerun, bar_save, find_close, find_query,
    find_step, gutter_hovered, gutter_pressed, paste_cancel, paste_confirm, restart_shell,
};
use super::ime::ImeGlue;
use super::launch::Launch;
use super::pointer::use_pane_mouse;
use super::slot::{Device, Slot, layout, repaint};
use super::status::StatusLine;
use super::window_help::{
    device_pixels, focus_in, place_cursor, send_text, set_focus, take_notice,
};
use crate::input::{Composing, RawKey, Repeat, stroke};
use temor_core::notify::Focus;
use temor_core::vocab::ids::BlockId;

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
    let hovered = use_signal(|| None::<BlockId>);

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
        move |_: FocusEvent| {
            set_focus(&slot, Focus::Focused);
            focus_in(ime.clone(), Rc::clone(&slot), rect)
        }
    };
    let on_blur = {
        let ime = ime.clone();
        let apply = apply.clone();
        let slot = Rc::clone(&slot);
        move |_: FocusEvent| {
            set_focus(&slot, Focus::Unfocused);
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
    let chrome = slot
        .borrow()
        .session
        .as_ref()
        .map_or_else(Chrome::default, |s| chrome_of(s, hovered()));
    let message = notice();
    // TODO(quire 0.2.31): full-height root (RootExtent::Window) and quire's layout components and
    // spacing tokens replace the inline layout below (CONSUMING.md rules 1 and 2).
    let pane_style =
        "position:relative; flex:1 1 auto; min-height:0; width:100%; outline:none; overflow:hidden";
    rsx! {
        Ds { appearance: Appearance::default(), material: Material::Window, extent: RootExtent::Window,
            // quire v0.3.1 has no column layout component (only SplitView, for panes), so the
            // window's column stays a plain flex element inside the window-high root (asked).
            div { style: "display:flex; flex-direction:column; width:100%; height:100%",
                if let Some((query, position)) = find {
                    FindBar {
                        query,
                        position,
                        on_query:find_query(Rc::clone(&slot), ui, started),
                        on_step:find_step(Rc::clone(&slot), ui),
                        on_close:find_close(Rc::clone(&slot), ui),
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
                    onpointerdown:gutter_pressed(Rc::clone(&slot), signals, ui, notice, mouse.down.clone()),
                    onpointerup: move |e| (mouse.up)(e),
                    onpointermove:gutter_hovered(Rc::clone(&slot), signals, hovered),
                    onwheel: move |e| (mouse.wheel)(e),
                    TextureLayer { texture: Some(texture.clone()), fit: TextureFit::Actual, sampling: Sampling::Nearest }
                    if let Some(text) = preedit() {
                        div { style: "position:absolute; left:0; bottom:0; white-space:pre; text-decoration:underline", "{text}" }
                    }
                    GutterMarks { marks: chrome.marks.clone() }
                }
                if let Some(bar) = chrome.bar.clone() {
                    BlockBar {
                        bar,
                        on_copy_command:bar_copy_command(Rc::clone(&slot), ui),
                        on_copy_output:bar_copy_output(Rc::clone(&slot), ui),
                        on_save:bar_save(Rc::clone(&slot), ui, notice),
                        on_rerun:bar_rerun(Rc::clone(&slot), ui),
                        on_close:bar_close(Rc::clone(&slot), ui),
                    }
                }
                StatusLine {
                    cwd: chrome.cwd.clone(),
                    title: chrome.title.clone(),
                    progress: chrome.progress,
                    bell: chrome.bell,
                }
                if let Some(reason) = pending {
                    PasteConfirm {
                        reason,
                        on_paste:paste_confirm(Rc::clone(&slot), ui),
                        on_cancel:paste_cancel(Rc::clone(&slot), ui),
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
                                onclick:restart_shell(Rc::clone(&slot), ui),
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

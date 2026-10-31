//! A viewer pane in the window (PLAN §10): anyview's pane in a tile, with temor's keys around it.
//! The pane takes plain keys only while it has the keyboard. Every chord with a modifier is the
//! window's: ⌘W closes the tile, the focus chords move out of it, ⌘T opens the bar.
//!
//! The pane asks the window for what it cannot do itself (close, give the keyboard back, open a
//! file in another program); `Desk::viewer_request` answers.

use std::cell::Cell;
use std::rc::Rc;

use anyview_pane::{PaneRequest, ViewerPane, use_pane_handle};
use dioxus::prelude::*;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::focus::select::Select;
use ds::focus::soon::focus_soon_told;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::workspace::WorkspaceIn;
use temor_panes::pane::keys::Action;

use crate::hub::{Desk, Origin};
use crate::input::{Routed, stroke};

/// One viewer pane.
#[component]
#[allow(non_snake_case)]
pub fn ViewerView(pane: PaneId, desk: Desk) -> Element {
    let handle = use_pane_handle();
    // The command bar lists the commands of the viewer that has the keyboard.
    use_hook({
        let desk = desk.clone();
        move || desk.hub.viewer_handle(pane, handle)
    });
    use_drop({
        let desk = desk.clone();
        move || desk.hub.viewer_handle_gone(pane)
    });
    let mut focused = use_signal({
        let desk = desk.clone();
        move || has_keyboard(&desk, pane)
    });
    {
        let desk = desk.clone();
        use_effect(move || {
            let _ = (desk.ui)();
            let has = has_keyboard(&desk, pane);
            if *focused.peek() != has {
                focused.set(has);
            }
        });
    }
    let on_key = {
        let desk = desk.clone();
        move |raw: RawKey| {
            if raw.phase == KeyPhase::Release {
                return;
            }
            if let Some(action) = stroke(&raw, false).and_then(|s| action_of(&desk, s)) {
                desk.run_action(action, Origin::Viewer);
            }
        }
    };
    let pressed = {
        let desk = desk.clone();
        move |_: PointerEvent| {
            if desk.hub.keyboard_pane() != Some(pane) {
                desk.act(WorkspaceIn::Focus(pane));
            }
        }
    };
    let _ = (desk.ui)();
    let shown = desk.hub.viewer_show(pane);
    let answer = desk.clone();
    let describes_only = desk.hub.viewer_describes_only(pane);
    let not_opened = desk.hub.viewer_not_opened(pane);
    let in_app = desk.clone();
    rsx! {
        div { class: "temor-pane",
            RawKeySurface {
                on_key: EventHandler::new(on_key),
                onpointerdown: EventHandler::new(pressed),
                common: Common {
                    aria_label: Some("File viewer".to_owned()),
                    extra_class: ExtraClass::parse("temor-keys").ok(),
                    ..Common::default()
                },
                if describes_only || not_opened {
                    InlineBanner {
                        severity: Severity::Info,
                        text: match not_opened {
                            true => "This file couldn't be opened here".to_owned(),
                            false => "This kind of file can't be shown here".to_owned(),
                        },
                        detail: Some(TextLine::from("Another app may be able to open it")),
                        actions: rsx! {
                            Button {
                                label: "Open in another app".to_owned(),
                                title: Some("Open it with the app your computer uses for this file".to_owned()),
                                onclick: move |_| in_app.viewer_open_in_app(pane),
                            }
                        },
                    }
                }
                {match shown {
                    Some(show) => rsx! {
                        ViewerPane {
                            file: show.file,
                            sequence: show.sequence,
                            edge: show.edge,
                            focused,
                            handle: Some(handle),
                            on_request: move |request: PaneRequest| answer.viewer_request(pane, request),
                        }
                    },
                    None => rsx! {
                        InlineBanner {
                            severity: Severity::Warn,
                            text: "This file cannot be shown.".to_owned(),
                            actions: rsx! {
                                Button {
                                    label: "Close".to_owned(),
                                    title: Some("Close this pane".to_owned()),
                                    onclick: {
                                        let desk = desk.clone();
                                        move |_| desk.act(WorkspaceIn::ClosePane(Some(pane)))
                                    },
                                }
                            },
                        }
                    },
                }}
            }
        }
    }
}

/// One of temor's own pages on a row's stage (`temor://history`) in the viewer pane `pane`: the
/// window's chords work in it as they do in a file viewer, a press gives it the keyboard, and
/// the keyboard given it (the page opened, or a step Back or Forward) moves into it.
#[component]
#[allow(non_snake_case)]
pub fn PageKeys(pane: PaneId, desk: Desk, label: String, children: Element) -> Element {
    let mut element = use_signal(|| None::<Rc<MountedData>>);
    let had = use_hook(|| Rc::new(Cell::new(false)));
    {
        let desk = desk.clone();
        use_effect(move || {
            let _ = (desk.ui)();
            match (has_keyboard(&desk, pane), element()) {
                (true, Some(element)) if !had.get() => {
                    had.set(true);
                    focus_soon_told(element, Select::None, EventHandler::new(|()| {}));
                }
                (false, _) => had.set(false),
                _ => {}
            }
        });
    }
    // A plain block, not a `RawKeySurface`: that one takes every key, and the page's own fields
    // (History's search) would get no typing. Only the window's chords are taken here.
    let on_key = {
        let desk = desk.clone();
        move |event: KeyboardEvent| {
            let raw = RawKey::new(
                KeyPhase::Press,
                (event.key(), event.code()),
                (event.modifiers(), event.location()),
            );
            if let Some(action) = stroke(&raw, false).and_then(|s| action_of(&desk, s)) {
                event.prevent_default();
                desk.run_action(action, Origin::Viewer);
            }
        }
    };
    let pressed = move |_: PointerEvent| {
        if desk.hub.keyboard_pane() != Some(pane) {
            desk.act(WorkspaceIn::Focus(pane));
        }
    };
    rsx! {
        div { class: "temor-pane",
            div {
                class: "ds-raw-keys temor-keys",
                role: "application",
                "aria-label": label,
                tabindex: "0",
                onmounted: move |event: MountedEvent| element.set(Some(event.data())),
                onkeydown: on_key,
                onpointerdown: pressed,
                {children}
            }
        }
    }
}

/// Whether the pane has the keyboard: it is the focused pane, and the command bar is not up.
fn has_keyboard(desk: &Desk, pane: PaneId) -> bool {
    desk.hub.keyboard_pane() == Some(pane) && !desk.hub.bar_is_open()
}

/// The window action a key is, if it is one: the keymap decides, as it does for a terminal. A key
/// that is not a chord of temor's is the pane's own and goes nowhere from here.
fn action_of(desk: &Desk, stroke: crate::input::Stroke) -> Option<Action> {
    match desk.hub.route_key(stroke) {
        Routed::Action(action) => Some(action),
        Routed::Shell(_) | Routed::Ignored => None,
    }
}

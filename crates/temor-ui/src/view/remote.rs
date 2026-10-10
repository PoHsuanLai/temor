//! What a pane on a host's helper shows (PLAN §9b, Tier B): the question about installing it, a
//! quiet line while the session is being set up or the connection dropped (with Retry once temor
//! stopped trying), and the ports the host listens on with what can be done about each.

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::prelude::*;
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_remote::PortAction;
use temor_ssh::Port;

use super::remote_words::{
    ASK_DETAIL, HELPER_ASK, ask_line, port_action_words, port_line, stage_line, typing_line,
};
use crate::hub::{Desk, PortRow, RemoteStage};

/// The helper banners of `pane`; nothing for a pane that is not on a helper.
#[component]
#[allow(non_snake_case)]
pub fn RemoteBanners(pane: PaneId, desk: Desk) -> Element {
    let _ = (desk.ui)();
    let Some(view) = desk.hub.remote_view(pane) else {
        return rsx! {};
    };
    let host = view.host.clone();
    let line = stage_line(view.stage, &host);
    let (yes, no, retry) = (desk.clone(), desk.clone(), desk.clone());
    rsx! {
        if view.stage == RemoteStage::Asking {
            InlineBanner {
                severity: Severity::Info,
                text: ask_line(&host),
                detail: Some(TextLine::from(ASK_DETAIL)),
                actions: rsx! {
                    Button {
                        label: HELPER_ASK.allow.to_owned(),
                        title: Some("Remember this for this computer".to_owned()),
                        onclick: move |_| yes.remote_answer(pane, Grant::Allowed),
                    }
                    Button {
                        label: HELPER_ASK.refuse.to_owned(),
                        title: Some("Use a plain connection; temor won't ask again".to_owned()),
                        onclick: move |_| no.remote_answer(pane, Grant::Refused),
                    }
                },
            }
        }
        if let Some(text) = line {
            div { class: "temor-host-line",
                Label { text, style: LabelStyle::Caption }
                if view.stage == RemoteStage::Lost {
                    Button {
                        label: "Retry".to_owned(),
                        title: Some("Connect again".to_owned()),
                        onclick: move |_| retry.remote_retry(pane),
                    }
                }
            }
        }
        if let Some(text) = typing_line(view.typing) {
            div { class: "temor-host-line",
                Label { text: text.to_owned(), style: LabelStyle::Caption }
            }
        }
        for row in view.ports.iter().cloned() {
            Listening { key: "{row.port}", pane, row, desk: desk.clone() }
        }
    }
}

/// One listening port and the buttons for what can be done about it.
#[component]
#[allow(non_snake_case)]
fn Listening(pane: PaneId, row: PortRow, desk: Desk) -> Element {
    let text = port_line(&row.program, row.port);
    rsx! {
        div { class: "temor-host-line",
            Label { text, style: LabelStyle::Caption }
            for action in row.actions.iter().cloned() {
                PortButton { key: "{port_action_words(&action).0}", pane, action, desk: desk.clone() }
            }
        }
    }
}

/// One thing to do about a port.
#[component]
#[allow(non_snake_case)]
fn PortButton(pane: PaneId, action: PortAction, desk: Desk) -> Element {
    let (label, tip) = port_action_words(&action);
    rsx! {
        Button {
            label,
            title: Some(tip.to_owned()),
            onclick: move |_| match &action {
                PortAction::OpenAt { port, .. } => {
                    if let Some(port) = Port::parse(&port.0.to_string()) {
                        desk.host_open_direct(pane, port);
                    }
                }
                PortAction::Forward { port } => desk.host_forward(pane, &port.0.to_string()),
            },
        }
    }
}

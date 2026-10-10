//! What a host pane shows above and below its terminal (PLAN §9b): the question being asked, the
//! browser check, and once connected, the forwarded ports and the files being edited, with the
//! fields to forward a port and to edit a file locally. A pane that is not a host shows nothing.

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::prelude::*;
use temor_core::host::edit::Resolution;
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_ssh::Port;

use super::host_words::{ask_words, edit_line, forward_line, stage_line};
use super::remote::RemoteBanners;
use crate::edge::ssh_work::EditId;
use crate::hub::{CheckRow, Desk, Direct, EditLine, EditStatus, ForwardLine, HostView, Stage};

/// The host banners of `pane`.
#[component]
#[allow(non_snake_case)]
pub fn HostBanners(pane: PaneId, desk: Desk) -> Element {
    let port = use_signal(String::new);
    let file = use_signal(String::new);
    let _ = (desk.ui)();
    let Some(view) = desk.hub.host_view(pane) else {
        return rsx! {};
    };
    let host = view.host.clone();
    rsx! {
        div { class: "temor-host",
            if let Stage::Asking(action) = view.stage {
                Question { pane, action, desk: desk.clone() }
            }
            if let Some(line) = stage_line(view.stage, &host) {
                Label { text: line, style: LabelStyle::Caption }
            }
            if view.stage == Stage::Failed {
                Button {
                    label: "Try again".to_owned(),
                    title: Some("Connect again".to_owned()),
                    onclick: {
                        let desk = desk.clone();
                        move |_| desk.restart(pane)
                    },
                }
            }
            if view.check == CheckRow::Waiting {
                InlineBanner {
                    severity: Severity::Info,
                    text: "Confirm it's you in your browser".to_owned(),
                    detail: Some(TextLine::from(format!("{host} asks for this before it connects"))),
                    actions: rsx! {
                        Button {
                            label: "Open".to_owned(),
                            title: Some("Open the confirmation page".to_owned()),
                            onclick: {
                                let desk = desk.clone();
                                move |_| desk.host_open_check(pane)
                            },
                        }
                    },
                }
            }
            RemoteBanners { pane, desk: desk.clone() }
            if view.stage == Stage::Up {
                Tools { pane, desk: desk.clone(), view: view.clone(), port, file }
            }
            for line in view.forwards.iter().copied() {
                Forwarded { key: "{line.remote.get()}", pane, host: host.clone(), line, desk: desk.clone() }
            }
            for (id, edit) in view.edits.iter().map(|edit| (edit.id.0, edit.clone())) {
                Edited { key: "{id}", edit, desk: desk.clone() }
            }
        }
    }
}

/// One question, answered Allow or Refuse; the next one (or the connection) follows.
#[component]
#[allow(non_snake_case)]
fn Question(
    pane: PaneId,
    action: temor_core::host::consent::BootstrapAction,
    desk: Desk,
) -> Element {
    let ask = ask_words(action);
    let (yes, no) = (desk.clone(), desk);
    rsx! {
        InlineBanner {
            severity: Severity::Info,
            text: ask.question.to_owned(),
            detail: Some(TextLine::from(ask.detail)),
            actions: rsx! {
                Button {
                    label: ask.allow.to_owned(),
                    title: Some("Remember this for this computer".to_owned()),
                    onclick: move |_| yes.host_answer(pane, Grant::Allowed),
                }
                Button {
                    label: ask.refuse.to_owned(),
                    title: Some("Remember this for this computer".to_owned()),
                    onclick: move |_| no.host_answer(pane, Grant::Refused),
                }
            },
        }
    }
}

/// The fields to forward a port and to edit a file.
#[component]
#[allow(non_snake_case)]
fn Tools(
    pane: PaneId,
    desk: Desk,
    view: HostView,
    port: Signal<String>,
    file: Signal<String>,
) -> Element {
    let (mut port, mut file) = (port, file);
    let forwarding = desk.clone();
    let direct = desk.clone();
    let editing = desk.clone();
    let typed = port();
    let opens_direct = view.direct == Direct::Available;
    rsx! {
        div { class: "temor-host-line",
            TextField {
                label: "Port on the computer".to_owned(),
                value: typed.clone(),
                placeholder: "Port".to_owned(),
                oninput: move |text: String| port.set(text),
            }
            Button {
                label: "Forward port…".to_owned(),
                title: Some("Reach this computer's port from here".to_owned()),
                onclick: move |_| forwarding.host_forward(pane, &typed),
            }
            if opens_direct {
                Button {
                    label: "Open directly".to_owned(),
                    title: Some("Open the port over your tailnet, with no forward".to_owned()),
                    onclick: move |_| {
                        if let Some(number) = Port::parse(port().trim()) {
                            direct.host_open_direct(pane, number);
                        }
                    },
                }
            }
        }
        div { class: "temor-host-line",
            TextField {
                label: "File on the computer".to_owned(),
                value: file(),
                placeholder: "File, like notes.txt or /etc/hosts".to_owned(),
                oninput: move |text: String| file.set(text),
            }
            Button {
                label: "Edit locally".to_owned(),
                title: Some("Open a copy here and save it back".to_owned()),
                onclick: move |_| editing.host_edit(pane, &file()),
            }
        }
    }
}

/// One forwarded port.
#[component]
#[allow(non_snake_case)]
fn Forwarded(pane: PaneId, host: String, line: ForwardLine, desk: Desk) -> Element {
    let ForwardLine { remote, local } = line;
    let text = forward_line(&host, remote.get(), local.map(Port::get));
    let (open, close) = (desk.clone(), desk);
    rsx! {
        div { class: "temor-host-line",
            Label { text, style: LabelStyle::Caption }
            if local.is_some() {
                Button {
                    label: "Open".to_owned(),
                    title: Some("Open it in your browser".to_owned()),
                    onclick: move |_| open.host_open_forwarded(pane, remote),
                }
            }
            Button {
                label: "Close".to_owned(),
                title: Some("Stop forwarding this port".to_owned()),
                onclick: move |_| close.host_cancel_forward(pane, remote),
            }
        }
    }
}

/// One file being edited; a changed file asks what to do.
#[component]
#[allow(non_snake_case)]
fn Edited(edit: EditLine, desk: Desk) -> Element {
    let text = edit_line(&edit.name, edit.status);
    let id: EditId = edit.id;
    let asking = matches!(
        edit.status,
        EditStatus::Changed | EditStatus::Rewritten | EditStatus::Deleted
    );
    let (done, over, theirs, later) = (desk.clone(), desk.clone(), desk.clone(), desk);
    if asking {
        return rsx! {
            InlineBanner {
                severity: Severity::Warn,
                text,
                detail: Some(TextLine::from("Your copy and the computer's file now differ")),
                actions: rsx! {
                    Button {
                        label: "Replace theirs".to_owned(),
                        title: Some("Write your copy over the computer's file".to_owned()),
                        onclick: move |_| over.host_resolve(id, Resolution::Overwrite),
                    }
                    Button {
                        label: "Use theirs".to_owned(),
                        title: Some("Discard your changes and fetch the computer's file".to_owned()),
                        onclick: move |_| theirs.host_resolve(id, Resolution::TakeRemote),
                    }
                    Button {
                        label: "Decide later".to_owned(),
                        title: Some("Keep editing; the next save asks again".to_owned()),
                        onclick: move |_| later.host_resolve(id, Resolution::Later),
                    }
                },
            }
        };
    }
    rsx! {
        div { class: "temor-host-line",
            Label { text, style: LabelStyle::Caption }
            Button {
                label: "Done".to_owned(),
                title: Some("Stop watching this copy".to_owned()),
                onclick: move |_| done.host_close_edit(id),
            }
        }
    }
}

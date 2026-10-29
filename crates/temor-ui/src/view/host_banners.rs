//! What a host pane shows above and below its terminal (PLAN §9b): the question being asked, the
//! browser check, and once connected, the forwarded ports and the files being edited, with the
//! fields to forward a port and to edit a file locally. A pane that is not a host shows nothing.

use dioxus::prelude::*;
use ds::components::content::label::LabelStyle;
use ds::prelude::*;
use temor_client::model::host::edit::Resolution;
use temor_client::model::remote::Grant;
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::Port;

use super::host_words::{
    CLOSE_FORWARD, DECIDE_LATER, DONE_EDITING, EDIT_LOCALLY, FORWARD_PORT, OPEN_CHECK,
    OPEN_DIRECTLY, OPEN_FORWARDED, REMEMBER_TIP, REPLACE_THEIRS, SHOW_CHANGES, TRY_AGAIN,
    TRY_ANYWAY, USE_THEIRS, ask_words, edit_line, forward_line, stage_line,
};
use super::remote::RemoteBanners;
use crate::hub::{Desk, EditLine, EditStatus, ForwardLine, HostView, Stage};
use temor_client::ssh::work::EditId;

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
            if view.stage == Stage::Offline {
                Button {
                    label: TRY_ANYWAY.label,
                    title: TRY_ANYWAY.title(),
                    onclick: {
                        let desk = desk.clone();
                        move |_| desk.host_try_anyway(pane)
                    },
                }
            }
            if view.stage == Stage::Failed {
                Button {
                    label: TRY_AGAIN.label,
                    title: TRY_AGAIN.title(),
                    onclick: {
                        let desk = desk.clone();
                        move |_| desk.restart(pane)
                    },
                }
            }
            if view.check_waiting {
                InlineBanner {
                    severity: Severity::Info,
                    text: "Confirm it's you in your browser".to_owned(),
                    detail: Some(TextLine::from(format!("{host} asks for this before it connects"))),
                    actions: rsx! {
                        Button {
                            label: OPEN_CHECK.label,
                            title: OPEN_CHECK.title(),
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
    action: temor_client::model::host::consent::BootstrapAction,
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
                    title: Some(REMEMBER_TIP.to_owned()),
                    onclick: move |_| yes.host_answer(pane, Grant::Allowed),
                }
                Button {
                    label: ask.refuse.to_owned(),
                    title: Some(REMEMBER_TIP.to_owned()),
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
    let opens_direct = view.direct_ports;
    rsx! {
        BannerActionRow {
            label: "Forward a port",
            field: rsx! {
                TextField {
                    label: "Port on the computer".to_owned(),
                    value: typed.clone(),
                    placeholder: "Port".to_owned(),
                    oninput: move |text: String| port.set(text),
                }
            },
            Button {
                label: FORWARD_PORT.label,
                title: FORWARD_PORT.title(),
                onclick: move |_| forwarding.host_forward(pane, &typed),
            }
            if opens_direct {
                Button {
                    label: OPEN_DIRECTLY.label,
                    title: OPEN_DIRECTLY.title(),
                    onclick: move |_| {
                        if let Some(number) = Port::parse(port().trim()) {
                            direct.host_open_direct(pane, number);
                        }
                    },
                }
            }
        }
        BannerActionRow {
            label: "Edit a file",
            field: rsx! {
                TextField {
                    label: "File on the computer".to_owned(),
                    value: file(),
                    placeholder: "File, like notes.txt or /etc/hosts".to_owned(),
                    oninput: move |text: String| file.set(text),
                }
            },
            Button {
                label: EDIT_LOCALLY.label,
                title: EDIT_LOCALLY.title(),
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
                    label: OPEN_FORWARDED.label,
                    title: OPEN_FORWARDED.title(),
                    onclick: move |_| open.host_open_forwarded(pane, remote),
                }
            }
            Button {
                label: CLOSE_FORWARD.label,
                title: CLOSE_FORWARD.title(),
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
    let (done, over, theirs, later) = (desk.clone(), desk.clone(), desk.clone(), desk.clone());
    let changes = desk;
    let diff_shown = edit.status != EditStatus::Deleted;
    if asking {
        return rsx! {
            InlineBanner {
                severity: Severity::Warn,
                text,
                detail: Some(TextLine::from("Your copy and the computer's file now differ")),
                actions: rsx! {
                    if diff_shown {
                        Button {
                            label: SHOW_CHANGES.label,
                            title: SHOW_CHANGES.title(),
                            onclick: move |_| changes.host_show_changes(id),
                        }
                    }
                    Button {
                        label: REPLACE_THEIRS.label,
                        title: REPLACE_THEIRS.title(),
                        onclick: move |_| over.host_resolve(id, Resolution::Overwrite),
                    }
                    Button {
                        label: USE_THEIRS.label,
                        title: USE_THEIRS.title(),
                        onclick: move |_| theirs.host_resolve(id, Resolution::TakeRemote),
                    }
                    Button {
                        label: DECIDE_LATER.label,
                        title: DECIDE_LATER.title(),
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
                label: DONE_EDITING.label,
                title: DONE_EDITING.title(),
                onclick: move |_| done.host_close_edit(id),
            }
        }
    }
}

//! The downloads shelf in the sidebar's foot (PLAN §7, §9): a disclosure that opens the files
//! that arrived, newest first. Each row opens its file on a press, and offers to show it in the
//! folder, to jump back to its tab, and to take it off the list (the file stays). A file that is
//! still arriving shows a bar.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::base::vocab::RowState;
use ds::components::content::label::LabelStyle;
use ds::components::controls::disclosure::Disclosure;
use ds::components::controls::progress::model::{Progress as Share, ProgressStyle};
use ds::components::controls::progress::view::ProgressIndicator;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::shelf::{
    Row as Download, RowAction as Act, RowState as Arriving, Upload, UploadState, upload_detail,
};

use crate::hub::Desk;

/// The Downloads disclosure for the foot of the sidebar.
#[component]
#[allow(non_snake_case)]
pub fn ShelfFoot(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let view = desk.hub.shelf_view();
    let uploads = desk.hub.uploads();
    let mut shown = use_signal(|| Shown::Hidden);
    let label = match view.unseen {
        0 => "Downloads".to_owned(),
        n => format!("Downloads ({n})"),
    };
    rsx! {
        div { class: "temor-dl-box",
            Disclosure {
                shown: shown(),
                on_toggle: move |next: Shown| shown.set(next),
                label: Some(label),
                for upload in uploads.iter().cloned() {
                    UploadRow { key: "up{upload.id.0}", upload, desk: desk.clone() }
                }
                if view.rows.is_empty() && uploads.is_empty() {
                    Label { text: view.empty, style: LabelStyle::Caption }
                }
                for (id, row) in view.rows.into_iter().map(|row| (row.id.0, row)) {
                    DownloadRow { key: "{id}", row, desk: desk.clone() }
                }
            }
        }
    }
}

/// One file on the shelf.
#[component]
#[allow(non_snake_case)]
fn DownloadRow(row: Download, desk: Desk) -> Element {
    let id = row.id;
    let can = |want: Act| row.actions.contains(&want);
    let tab = row.actions.iter().find_map(|action| match action {
        Act::ShowInTab(tab) => Some(*tab),
        _ => None,
    });
    let act = {
        let desk = desk.clone();
        move |action: Act| {
            for text in desk.hub.shelf_act(id, action) {
                desk.say(text);
            }
            desk.bump();
        }
    };
    let remove = can(Act::Clear).then(|| {
        let act = act.clone();
        RowAction::new(
            Icon::X,
            "Remove from the list",
            EventHandler::new(move |_: Press| act(Act::Clear)),
        )
    });
    let open = can(Act::Open).then(|| {
        let act = act.clone();
        EventHandler::new(move |_: Press| act(Act::Open))
    });
    let percent = match &row.state {
        Arriving::Receiving { percent } => *percent,
        _ => None,
    };
    let detail = match &row.state {
        Arriving::Failed { message } => message.clone(),
        Arriving::Receiving { percent: None } | Arriving::Waiting => {
            format!("Receiving, {}", row.detail)
        }
        _ => row.detail.clone(),
    };
    rsx! {
        div { class: "temor-dl",
            Row {
                leading: RowLeading::Icon(Icon::Download),
                title: row.title.clone(),
                detail: Some(TextLine::from(detail)),
                state: RowState::default(),
                action: remove,
                onclick: open,
            }
            if let Some(percent) = percent {
                ProgressIndicator {
                    style: ProgressStyle::Bar,
                    progress: Share::Known(Fraction(u16::from(percent) * 10)),
                    common: Common { aria_label: Some("Download progress".to_owned()), ..Common::default() },
                }
            }
            div { class: "temor-dl-actions",
                if can(Act::Reveal) {
                    Button {
                        label: "Show in folder".to_owned(),
                        size: ControlSize::Small,
                        title: Some("Show the file in your files".to_owned()),
                        onclick: {
                            let act = act.clone();
                            move |_: Press| act(Act::Reveal)
                        },
                    }
                }
                if let Some(tab) = tab {
                    Button {
                        label: "Show tab".to_owned(),
                        size: ControlSize::Small,
                        title: Some("Go back to the tab it came from".to_owned()),
                        onclick: {
                            let act = act.clone();
                            move |_: Press| act(Act::ShowInTab(tab))
                        },
                    }
                }
            }
        }
    }
}

/// One file on its way to another computer.
#[component]
#[allow(non_snake_case)]
fn UploadRow(upload: Upload, desk: Desk) -> Element {
    let id = upload.id;
    let remove = (upload.state != UploadState::Sending).then(|| {
        RowAction::new(
            Icon::X,
            "Remove from the list",
            EventHandler::new(move |_: Press| {
                desk.hub.clear_upload(id);
                desk.bump();
            }),
        )
    });
    rsx! {
        div { class: "temor-dl",
            Row {
                leading: RowLeading::Icon(Icon::Upload),
                title: upload.name.clone(),
                detail: Some(TextLine::from(upload_detail(&upload))),
                state: RowState::default(),
                action: remove,
            }
        }
    }
}

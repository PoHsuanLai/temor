//! Downloads (PLAN §7, §9, P23): a button at the right end of the sidebar's head, with a dot while
//! a file arrives or one arrived unopened, that opens the page `temor://downloads` on the row's
//! stage, as the command bar's Downloads and its chord do. The page lists the files that arrived,
//! newest first, under those on their way to other computers. A press opens a file; beside it are
//! Show in Folder, Paste Path into the terminal the person was using (or Send to the computer
//! that terminal is on), and Show Tab, the tab it came from; its X takes it off the list (the file
//! stays). A secondary press is the one menu. A file that is still arriving shows a bar.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::RowState;
use ds::components::controls::progress::model::{Progress as Share, ProgressStyle};
use ds::components::controls::progress::view::ProgressIndicator;
use ds::components::overlays::empty_state::EmptyForm;
use ds::host::measure::Anchor;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::model::menu::{DownloadFacts, Item, Target};
use temor_client::model::shelf::{
    Row as Download, RowAction as Act, RowState as Arriving, Upload, UploadState, upload_detail,
};

use super::menu::{Pick, menu_items};
use super::names::named;
use super::shelf_words::{ActWords, REMOVE_FROM_LIST, act_words};
use super::tool_button::ToolButton;
use crate::hub::Desk;
use temor_panes::pane::keys::Action;

/// The Downloads button for the head of the sidebar.
#[component]
#[allow(non_snake_case)]
pub fn ShelfButton(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let view = desk.hub.shelf_view();
    let going = !desk.hub.uploads().is_empty()
        || view
            .rows
            .iter()
            .any(|row| !matches!(row.state, Arriving::Ready));
    let dot = match (going, view.unseen) {
        (true, _) => Some("temor-dl-dot going"),
        (false, 0) => None,
        (false, _) => Some("temor-dl-dot"),
    };
    let opening = desk.clone();
    rsx! {
        span { class: "temor-dl-hold",
            ToolButton {
                icon: Icon::Download,
                label: Action::ShowDownloads.label(),
                tip: desk.hub.tooltip(Action::ShowDownloads.label(), Action::ShowDownloads),
                onclick: move |()| opening.open_downloads(),
            }
            if let Some(class) = dot {
                span { class, aria_hidden: "true" }
            }
        }
    }
}

/// The page `temor://downloads`.
#[component]
#[allow(non_snake_case)]
pub fn DownloadsPage(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let view = desk.hub.shelf_view();
    let uploads = desk.hub.uploads();
    rsx! {
        div { class: "temor-downloads", role: "list", "aria-label": "Downloads",
            if view.rows.is_empty() && uploads.is_empty() {
                EmptyState {
                    form: EmptyForm::Empty,
                    title: "No downloads",
                    description: Some(TextLine::from(view.empty)),
                }
            }
            for upload in uploads {
                UploadRow { key: "up{upload.id.0}", upload, desk: desk.clone() }
            }
            for (id, row) in view.rows.into_iter().map(|row| (row.id.0, row)) {
                DownloadRow { key: "{id}", row, desk: desk.clone() }
            }
        }
    }
}

/// One file that arrived, or is arriving.
#[component]
#[allow(non_snake_case)]
fn DownloadRow(row: Download, desk: Desk) -> Element {
    let id = row.id;
    let can = |want: Act| row.actions.contains(&want);
    // The tab it came from, unless that is the tab in front.
    let front = desk.hub.front().map(|(tab, ..)| tab);
    let tab = row
        .actions
        .iter()
        .find_map(|action| match action {
            Act::ShowInTab(tab) => Some(*tab),
            _ => None,
        })
        .filter(|tab| Some(*tab) != front);
    let act = {
        let desk = desk.clone();
        move |action: Act| desk.shelf_act(id, action)
    };
    let remove = can(Act::Clear).then(|| {
        let act = act.clone();
        RowAction::new(
            Icon::X,
            REMOVE_FROM_LIST,
            EventHandler::new(move |_: Press| act(Act::Clear)),
        )
    });
    let open = can(Act::Open).then(|| {
        let act = act.clone();
        EventHandler::new(move |press: Press| {
            // A secondary press is the file's menu (`oncontextmenu` below).
            if !matches!(press.button, PointerButton::Secondary) {
                act(Act::Open);
            }
        })
    });
    let target = desk.hub.shelf_target();
    let words = |action: Act| act_words(action, &target);
    let give = can(Act::UseInPane).then(|| words(Act::UseInPane)).flatten();
    let mut menu = use_signal(|| None::<Point>);
    let facts = DownloadFacts {
        ready: can(Act::Open),
        from_tab: tab.is_some(),
        clearable: can(Act::Clear),
    };
    // Paste Path (or Send to) is the file's own: its words follow where the terminal is.
    let own: Vec<MenuItem<Act>> = give
        .iter()
        .map(|ActWords { label, .. }| MenuItem::new(Act::UseInPane, label.clone()))
        .collect();
    let items = menu_items(Target::Download(facts), false, own);
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
    let button = |action: Act, words: ActWords| {
        let act = act.clone();
        rsx! {
            Button {
                label: words.label,
                size: ControlSize::Small,
                title: Some(words.tip),
                onclick: move |_: Press| act(action),
            }
        }
    };
    let reveal = words(Act::Reveal).filter(|_| can(Act::Reveal));
    let show_tab = tab.and_then(|tab| words(Act::ShowInTab(tab)).map(|words| (tab, words)));
    let picking = act.clone();
    rsx! {
        div {
            class: "temor-dl",
            role: "listitem",
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let at = event.client_coordinates();
                menu.set(Some(Point {
                    x: Px(at.x as f32),
                    y: Px(at.y as f32),
                }));
            },
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
                    common: named("Download progress"),
                }
            }
            div { class: "temor-dl-actions",
                if let Some(words) = reveal {
                    {button(Act::Reveal, words)}
                }
                if let Some(words) = give {
                    {button(Act::UseInPane, words)}
                }
                if let Some((tab, words)) = show_tab {
                    {button(Act::ShowInTab(tab), words)}
                }
            }
        }
        if let Some(at) = menu() {
            Menu {
                placement: MenuPlacement::Context,
                anchor: Anchor::Point(at),
                items,
                onpick: move |pick: Pick<Act>| match pick {
                    Pick::Item(Item::Open) => picking(Act::Open),
                    Pick::Item(Item::ShowInFolder) => picking(Act::Reveal),
                    Pick::Item(Item::ShowTab) => {
                        if let Some(tab) = tab {
                            picking(Act::ShowInTab(tab));
                        }
                    }
                    Pick::Item(Item::RemoveFromList) => picking(Act::Clear),
                    Pick::Own(action) => picking(action),
                    Pick::Item(_) => {}
                },
                onclose: move |()| menu.set(None),
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
            REMOVE_FROM_LIST,
            EventHandler::new(move |_: Press| {
                desk.hub.clear_upload(id);
                desk.bump();
            }),
        )
    });
    rsx! {
        div { class: "temor-dl", role: "listitem",
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

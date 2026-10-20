//! The downloads shelf in the sidebar's head (PLAN §7, §9), as mailo's is: a button at the head's
//! right end, with a dot while a file arrives or one arrived unopened, that opens the files that
//! arrived, newest first, in a popover below it. The command bar's Downloads opens it too. Each row opens its file on a press, and offers to show it in the
//! folder, to paste its path into the terminal the person was using (or send the file there when
//! that terminal is on another computer), to jump back to its tab, and to take it off the list
//! (the file stays). A secondary press opens all of these, and Copy Path, as a menu. A file that
//! is still arriving shows a bar.

use dioxus::prelude::*;
use ds::base::geometry::placement::{Align, Side};
use ds::base::press::Press;
use ds::base::vocab::RowState;
use ds::components::content::label::{LabelRole, LabelStyle};
use ds::components::controls::progress::model::{Progress as Share, ProgressStyle};
use ds::components::controls::progress::view::ProgressIndicator;
use ds::components::overlays::popover::Arrow;
use ds::host::measure::{Anchor, MountedRef, use_rect};
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::shelf::{
    Row as Download, RowAction as Act, RowState as Arriving, Upload, UploadState, upload_detail,
};

use super::names::named;
use super::shelf_words::{ActWords, REMOVE_FROM_LIST, act_words};
use super::tool_button::ToolButton;
use crate::hub::Desk;
use temor_pane::keys::Action;

/// The Downloads button for the head of the sidebar, and its list while it is open.
#[component]
#[allow(non_snake_case)]
pub fn ShelfButton(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let view = desk.hub.shelf_view();
    let uploads = desk.hub.uploads();
    let mut shown = desk.shelf;
    let toggling = desk.clone();
    let mut hold = use_signal(|| None::<MountedRef>);
    let going = !uploads.is_empty()
        || view
            .rows
            .iter()
            .any(|row| !matches!(row.state, Arriving::Ready));
    let dot = match (going, view.unseen) {
        (true, _) => Some("temor-dl-dot going"),
        (false, 0) => None,
        (false, _) => Some("temor-dl-dot"),
    };
    let anchor = hold().map_or(Anchor::Point(Point::default()), Anchor::Mounted);
    rsx! {
        span {
            class: "temor-dl-hold",
            onmounted: move |event: MountedEvent| hold.set(Some(MountedRef(event.data()))),
            ToolButton {
                icon: Icon::Download,
                label: Action::ShowDownloads.label(),
                tip: desk.hub.tooltip(Action::ShowDownloads.label(), Action::ShowDownloads),
                // The same path as the chord and the bar: it shows a sidebar that is only peeking
                // before it opens the list.
                onclick: move |()| {
                    let _ = toggling.window_action(Action::ShowDownloads);
                },
            }
            if let Some(class) = dot {
                span { class, aria_hidden: "true" }
            }
            if shown() == Shown::Visible {
                Popover {
                    anchor,
                    placement: Placement::new(Side::Bottom, Align::End),
                    gap: Px(6.0),
                    arrow: Arrow::None,
                    common: named(Action::ShowDownloads.label()),
                    onclose: move |()| shown.set(Shown::Hidden),
                    div { class: "temor-dl-list",
                        Label { text: Action::ShowDownloads.label(), role: LabelRole::Secondary }
                        for upload in uploads.iter().cloned() {
                            UploadRow { key: "up{upload.id.0}", upload, desk: desk.clone() }
                        }
                        if view.rows.is_empty() && uploads.is_empty() {
                            Label { text: view.empty, style: LabelStyle::Footnote }
                        }
                        for (id, row) in view.rows.into_iter().map(|row| (row.id.0, row)) {
                            DownloadRow { key: "{id}", row, desk: desk.clone() }
                        }
                    }
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
            REMOVE_FROM_LIST,
            EventHandler::new(move |_: Press| act(Act::Clear)),
        )
    });
    let open = can(Act::Open).then(|| {
        let act = act.clone();
        EventHandler::new(move |_: Press| act(Act::Open))
    });
    // Use in pane puts the list away, so the person is back in the terminal it typed into.
    let mut shown = desk.shelf;
    let mut give = {
        let act = act.clone();
        move || {
            act(Act::UseInPane);
            shown.set(Shown::Hidden);
        }
    };
    let target = desk.hub.shelf_target();
    let words = |action: Act| act_words(action, &target);
    let give_button = can(Act::UseInPane).then(|| words(Act::UseInPane)).flatten();
    let rect = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    let has_menu = !row.actions.is_empty();
    let mut items = Vec::new();
    for action in &row.actions {
        let Some(ActWords { label, .. }) = words(*action) else {
            continue;
        };
        if *action == Act::Clear {
            items.push(MenuItem::Separator);
        }
        items.push(MenuItem::new(*action, label));
    }
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
        div {
            class: "temor-dl",
            onmounted: move |event: MountedEvent| rect.on_mounted(event),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                if has_menu {
                    menu.set(Shown::Visible);
                }
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
                if let Some(ActWords { label, tip }) = words(Act::Reveal).filter(|_| can(Act::Reveal)) {
                    Button {
                        label,
                        size: ControlSize::Small,
                        title: Some(tip),
                        onclick: {
                            let act = act.clone();
                            move |_: Press| act(Act::Reveal)
                        },
                    }
                }
                if let Some(ActWords { label, tip }) = give_button {
                    Button {
                        label,
                        size: ControlSize::Small,
                        title: Some(tip),
                        onclick: {
                            let mut give = give.clone();
                            move |_: Press| give()
                        },
                    }
                }
                if let Some((tab, ActWords { label, tip })) = tab.and_then(|tab| {
                    words(Act::ShowInTab(tab)).map(|w| (tab, w))
                }) {
                    Button {
                        label,
                        size: ControlSize::Small,
                        title: Some(tip),
                        onclick: {
                            let act = act.clone();
                            move |_: Press| act(Act::ShowInTab(tab))
                        },
                    }
                }
            }
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            Menu {
                placement: MenuPlacement::Context,
                anchor: Anchor::Rect(at),
                items,
                onpick: move |pick: Act| match pick {
                    Act::UseInPane => give(),
                    pick => act(pick),
                },
                onclose: move |()| menu.set(Shown::Hidden),
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

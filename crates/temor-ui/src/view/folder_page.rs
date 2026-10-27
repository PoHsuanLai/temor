//! A folder's page (PLAN §7, v14, `temor://local/<path>/`): the tabs open in the folder, then
//! what it holds, as Finder lists it (`temor_core::folder`), each with when it last changed. A tab
//! comes to the front on a press; a folder opens its own page (Back returns), and a file opens on
//! the stage as `temor view` would show it.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::components::overlays::empty_state::EmptyForm;
use ds::prelude::*;
use temor_core::folder::{Entry, listed, modified_words};

use super::tab_glyph::row_leading;
use crate::edge::folder_read::read_folder;
use crate::edge::local_zone::{local_zone, wall_now};
use crate::hub::{Desk, TabRow, folder_url, select_tab};

/// The page of `folder`, an absolute path ending in `/`.
#[component]
#[allow(non_snake_case)]
pub fn FolderPage(folder: String, desk: Desk) -> Element {
    let _ = (desk.ui)();
    // Read once each time the page is shown: a folder is not watched.
    let read = use_hook({
        let folder = folder.clone();
        move || read_folder(Path::new(&folder)).map(listed).ok()
    });
    let here = tabs_in(&desk, &folder);
    let zone = local_zone();
    let today = zone.day_of(wall_now());
    let body = match &read {
        None => rsx! {
            EmptyState {
                form: EmptyForm::NoResults,
                title: "This folder can't be read",
                description: Some(TextLine::from("It may have been moved, or temor may not be allowed to read it.")),
            }
        },
        Some((entries, _)) if entries.is_empty() => rsx! {
            EmptyState { form: EmptyForm::Empty, title: "This folder is empty" }
        },
        Some((entries, more)) => rsx! {
            div { class: "temor-history-day",
                SectionHeader { title: "Files".to_owned() }
                div { class: "temor-history-list",
                    for entry in entries.iter().cloned() {
                        EntryRow {
                            key: "{entry.name}",
                            path: Path::new(&folder).join(&entry.name),
                            entry,
                            when_today: today,
                            desk: desk.clone(),
                        }
                    }
                }
                if *more > 0 {
                    span { class: "temor-history-about", "and {more} more" }
                }
            }
        },
    };
    rsx! {
        div { class: "temor-history temor-folder",
            if !here.is_empty() {
                div { class: "temor-history-day",
                    SectionHeader { title: "Tabs Here".to_owned() }
                    div { class: "temor-history-list",
                        for row in here {
                            TabHere { key: "{row.id.0}", row, desk: desk.clone() }
                        }
                    }
                }
            }
            {body}
        }
    }
}

/// The tabs whose shell is in `folder`.
fn tabs_in(desk: &Desk, folder: &str) -> Vec<TabRow> {
    let folder = folder.trim_end_matches('/');
    let rows = desk.hub.sidebar();
    rows.pinned
        .into_iter()
        .chain(rows.loose)
        .chain(rows.folders.into_iter().flat_map(|(_, rows)| rows))
        .filter(|row| {
            row.place
                .as_deref()
                .map(|place| place.trim_end_matches('/'))
                == Some(folder)
        })
        .collect()
}

/// A tab open in the folder.
#[component]
#[allow(non_snake_case)]
fn TabHere(row: TabRow, desk: Desk) -> Element {
    let id = row.id;
    rsx! {
        Row {
            leading: row_leading(&row.icon),
            title: row.title.clone(),
            onclick: move |_: Press| desk.act(select_tab(id)),
        }
    }
}

/// A file or folder in the folder.
#[component]
#[allow(non_snake_case)]
fn EntryRow(entry: Entry, path: PathBuf, when_today: i64, desk: Desk) -> Element {
    let zone = local_zone();
    let when = entry
        .modified
        .map(|at| modified_words(at, zone, when_today))
        .unwrap_or_default();
    let folder = entry.folder;
    rsx! {
        Row {
            leading: RowLeading::Icon(match folder {
                true => Icon::Folder,
                false => Icon::File,
            }),
            title: entry.name.clone(),
            accessory: Accessory::Text(when),
            onclick: move |_: Press| match folder {
                true => desk.open_page(folder_url(&path.to_string_lossy())),
                false => desk.open_file(&path),
            },
        }
    }
}

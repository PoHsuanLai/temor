//! The ⌘T command bar (PLAN §7): quire's `CommandPalette` over the rows temor-core ranked. The
//! palette draws and walks them; it never re-ranks. The hub keeps the query and the highlight
//! (quire's palette machine, `bar::BarModel`), so this view only reports what the person did.
//!
//! While a viewer pane has the keyboard its own commands (Zoom to Fit, Rotate Left, …) are listed
//! after the bar's rows (PLAN §10). They are the pane's, not the bar's: the bar does not rank
//! them, the query only narrows them by name, and a picked one is run by the pane's handle. A row
//! there, "Find in This File", turns the field into a search of the open file (`bar::viewer_find`).

use std::cell::Cell;
use std::rc::Rc;

use anyview_pane::{HitList, PaneCommand};
use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::{
    GroupEntries, PaletteGroup, PaletteGroups, PaletteRow,
};
use ds::prelude::{AfterPick, CommandPalette, PaletteClose, RowChord};
use temor_core::bar::{BarRow, Section};
use temor_core::vocab::text::PluginId;

use crate::bar::{
    Escape, FIND_PLACEHOLDER, Finding, Reply, action_of, entry_row, escape, lift, reply,
};
use crate::hub::{BarData, Desk, HubRef};

/// What a row of the bar yields when it is picked.
#[derive(Debug, Clone, PartialEq)]
enum Pick {
    /// One of the bar's own rows.
    Bar(BarRow),
    /// A command of the viewer that has the keyboard, or one of its find's matches.
    Pane(PaneCommand),
    /// "Find in This File": the field searches the open file from now on.
    Find,
}

/// The bar, over the window.
#[component]
#[allow(non_snake_case)]
pub fn BarView(data: BarData, desk: Desk) -> Element {
    let (typed, selected, picked, closed) =
        (desk.clone(), desk.clone(), desk.clone(), desk.clone());
    let handle = desk.hub.keyboard_viewer();
    // ⌘F in a viewer opens the bar already searching that file.
    let wanted = use_hook(|| desk.hub.take_finding_wanted());
    let mut finding = use_signal(|| wanted);
    let mut list = use_signal(|| match wanted {
        Finding::On => HitList::Brief,
        Finding::Off => HitList::default(),
    });
    // The viewer whose find is up; the bar putting it away is the bar going, however it goes.
    let searched = use_hook(|| Rc::new(Cell::new(handle.filter(|_| wanted == Finding::On))));
    use_drop({
        let searched = Rc::clone(&searched);
        move || {
            if let Some(handle) = searched.get() {
                handle.end_find();
            }
        }
    });
    let searching = handle.filter(|_| finding() == Finding::On);
    let stepped = Rc::clone(&searched);
    let (groups, extra) = match (searching, handle) {
        (Some(handle), _) => {
            let group = lifted(handle.hits(list()), |_| true, None);
            let rows = rows_in(&group);
            (vec![group], rows)
        }
        (None, Some(handle)) => {
            let mut groups = groups_of(&data.rows, &desk.hub);
            let wanted = data.query.trim().to_lowercase();
            let group = lifted(
                handle.commands(),
                |row| row.value.label().to_lowercase().contains(&wanted),
                entry_row(Pick::Find, &data.query),
            );
            let rows = rows_in(&group);
            groups.push(group);
            (groups, rows)
        }
        (None, None) => (groups_of(&data.rows, &desk.hub), 0),
    };
    let placeholder = match searching {
        Some(_) => FIND_PLACEHOLDER,
        None => "Search tabs, Spaces and commands",
    };
    rsx! {
        CommandPalette::<Pick> {
            label: "Command bar",
            placeholder,
            query: data.query.clone(),
            tokens: Vec::<String>::new(),
            groups: PaletteGroups(groups),
            empty: "Nothing matches.",
            oninput: move |text: String| {
                if let Some(handle) = searching {
                    handle.find(text.clone());
                }
                typed.bar_typed(text);
            },
            on_select: Some(EventHandler::new(move |index: usize| selected.bar_select_among(index, extra))),
            selected: Some(data.selected),
            onpick: move |pick: Pick| match pick {
                Pick::Bar(row) => picked.bar_pick(&row.key),
                Pick::Find => {
                    searched.set(handle);
                    finding.set(Finding::On);
                    list.set(HitList::Brief);
                    picked.bar_typed(String::new());
                }
                Pick::Pane(command) => match searching {
                    // A match picked leaves the find up: the pane goes to it, and the bar stays.
                    Some(handle) => match reply(&command) {
                        Reply::ListAll => list.set(HitList::Whole),
                        Reply::Go => handle.run(command),
                    },
                    None => {
                        // The bar goes first, so the keyboard is the pane's again when it runs.
                        picked.bar_close();
                        if let Some(handle) = handle {
                            handle.run(command);
                        }
                    }
                },
            },
            // Escape leaves the file's find first, back to the bar's rows; a second one closes.
            // A click outside closes the bar at once, and ends a find that was running.
            onclose: move |how: PaletteClose| match escape(finding(), how) {
                Escape::Close => {
                    if let Some(handle) = stepped.take() {
                        handle.end_find();
                    }
                    closed.bar_close();
                }
                Escape::StepBack => {
                    if let Some(handle) = stepped.take() {
                        handle.end_find();
                    }
                    finding.set(Finding::Off);
                    list.set(HitList::default());
                    closed.bar_typed(String::new());
                }
            },
        }
    }
}

/// The pane's group as a group of the bar: the rows `keep` lets through, after `first`.
fn lifted(
    group: PaletteGroup<PaneCommand>,
    keep: impl Fn(&PaletteRow<PaneCommand>) -> bool,
    first: Option<PaletteRow<Pick>>,
) -> PaletteGroup<Pick> {
    let PaletteGroup {
        title,
        entries,
        action,
    } = group;
    let rows = match entries {
        GroupEntries::List(rows) => rows,
        GroupEntries::Grid(_) => Vec::new(),
    };
    let kept = rows.into_iter().filter(|row| keep(row)).collect();
    let rows = first
        .into_iter()
        .chain(lift(kept, Pick::Pane))
        .collect::<Vec<_>>();
    PaletteGroup {
        title,
        entries: GroupEntries::List(rows),
        action,
    }
}

/// How many rows `group` lists.
fn rows_in(group: &PaletteGroup<Pick>) -> usize {
    match &group.entries {
        GroupEntries::List(rows) => rows.len(),
        GroupEntries::Grid(_) => 0,
    }
}

/// The rows as the palette's groups, in the order they were ranked: a run of rows under one
/// header is one group, so the palette's numbering is the rows' own. A row that is also a pane
/// action shows its chord at its right.
fn groups_of(rows: &[BarRow], hub: &HubRef) -> Vec<PaletteGroup<Pick>> {
    runs(rows)
        .into_iter()
        .map(|(section, run)| {
            let lines = run
                .into_iter()
                .map(|row| PaletteRow {
                    chord: action_of(&row.action)
                        .and_then(|action| hub.shortcut_of(action))
                        .map(RowChord::always)
                        .unwrap_or_default(),
                    // The palette stays up until the bar's own step closes it, so the pick
                    // reaches the palette machine once.
                    after: AfterPick::KeepOpen,
                    ..PaletteRow::new(Pick::Bar(row.clone()), row.title.clone())
                })
                .collect();
            PaletteGroup::list(title_of(&section, |id| hub.plugin_name_of(id)), lines)
        })
        .collect()
}

/// The rows split where the section changes, each run with its section.
fn runs(rows: &[BarRow]) -> Vec<(Section, Vec<&BarRow>)> {
    let mut runs: Vec<(Section, Vec<&BarRow>)> = Vec::new();
    for row in rows {
        let section = Section::of(row);
        match runs.last_mut() {
            Some((last, run)) if *last == section => run.push(row),
            _ => runs.push((section, vec![row])),
        }
    }
    runs
}

fn title_of(section: &Section, name_of: impl Fn(&PluginId) -> String) -> String {
    match (section.title(), section) {
        (Some(fixed), _) => fixed.to_owned(),
        (None, Section::Plugin(plugin)) => name_of(plugin),
        (None, _) => String::new(),
    }
}

#[cfg(test)]
#[path = "bar_tests.rs"]
mod tests;

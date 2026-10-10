//! The ⌘T command bar (PLAN §7): quire's `CommandPalette` over the rows temor-core ranked. The
//! palette draws and walks them; it never re-ranks. The hub keeps the query and the highlight
//! (quire's palette machine, `bar::BarModel`), so this view only reports what the person did.
//!
//! While a viewer pane has the keyboard its own commands (Zoom to Fit, Rotate Left, …) are listed
//! after the bar's rows (PLAN §10). They are the pane's, not the bar's: the bar does not rank
//! them, the query only narrows them by name, and a picked one is run by the pane's handle.

use anyview_pane::{PaneCommand, PaneHandle};
use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::{
    GroupEntries, PaletteGroup, PaletteGroups, PaletteRow,
};
use ds::prelude::{AfterPick, CommandPalette, TextLine};
use temor_core::bar::{BarAction, BarRow, RowGroup, Target};

use crate::hub::{BarData, Desk};

/// What a row of the bar yields when it is picked.
#[derive(Debug, Clone, PartialEq)]
enum Pick {
    /// One of the bar's own rows.
    Bar(BarRow),
    /// A command of the viewer that has the keyboard.
    Pane(PaneCommand),
}

/// The bar, over the window.
#[component]
#[allow(non_snake_case)]
pub fn BarView(data: BarData, desk: Desk) -> Element {
    let (typed, selected, picked, closed) =
        (desk.clone(), desk.clone(), desk.clone(), desk.clone());
    let handle = desk.hub.keyboard_viewer();
    let mut groups = groups_of(&data.rows);
    let extra = match handle {
        Some(handle) => {
            let group = pane_group(handle, &data.query);
            let rows = rows_in(&group);
            groups.push(group);
            rows
        }
        None => 0,
    };
    rsx! {
        CommandPalette::<Pick> {
            label: "Command bar",
            placeholder: "Search tabs, Spaces and commands",
            query: data.query.clone(),
            tokens: Vec::<String>::new(),
            groups: PaletteGroups(groups),
            empty: "Nothing matches.",
            oninput: move |text: String| typed.bar_typed(text),
            on_select: Some(EventHandler::new(move |index: usize| selected.bar_select_among(index, extra))),
            selected: Some(data.selected),
            onpick: move |pick: Pick| match pick {
                Pick::Bar(row) => picked.bar_pick(&row.key),
                Pick::Pane(command) => {
                    // The bar goes first, so the keyboard is the pane's again when it runs.
                    picked.bar_close();
                    if let Some(handle) = handle {
                        handle.run(command);
                    }
                }
            },
            onclose: move |()| closed.bar_close(),
        }
    }
}

/// The viewer's commands as a group of the bar, narrowed to those whose name holds the query.
fn pane_group(handle: PaneHandle, query: &str) -> PaletteGroup<Pick> {
    let PaletteGroup {
        title,
        entries,
        action,
    } = handle.commands();
    let wanted = query.trim().to_lowercase();
    let rows = match entries {
        GroupEntries::List(rows) => rows,
        GroupEntries::Grid(_) => Vec::new(),
    };
    let rows = rows
        .into_iter()
        .filter(|row| row.value.label().to_lowercase().contains(&wanted))
        .map(|row| PaletteRow {
            value: Pick::Pane(row.value),
            title: row.title,
            detail: row.detail,
            leading: row.leading,
            accessory: row.accessory,
            chord: row.chord,
            availability: row.availability,
            action: row.action,
            // The bar closes itself on the pick, then the pane runs the command.
            after: AfterPick::KeepOpen,
            shape: row.shape,
        })
        .collect();
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

/// The rows as the palette's groups, in the order they were ranked: a run of rows from one rule
/// is one group, so the palette's numbering is the rows' own.
fn groups_of(rows: &[BarRow]) -> Vec<PaletteGroup<Pick>> {
    let mut groups: Vec<(RowGroup, Vec<PaletteRow<Pick>>)> = Vec::new();
    for row in rows {
        let line = PaletteRow {
            detail: kind_of(&row.action).map(TextLine::from),
            // The palette stays up until the bar's own step closes it, so the pick reaches the
            // palette machine once.
            after: AfterPick::KeepOpen,
            ..PaletteRow::new(Pick::Bar(row.clone()), row.title.clone())
        };
        match groups.last_mut() {
            Some((group, lines)) if *group == row.group => lines.push(line),
            _ => groups.push((row.group, vec![line])),
        }
    }
    groups
        .into_iter()
        .map(|(group, lines)| PaletteGroup::list(title_of(group), lines))
        .collect()
}

fn title_of(group: RowGroup) -> &'static str {
    match group {
        RowGroup::Run => "Start",
        RowGroup::Match => "Go to",
        RowGroup::Plugin => "More",
        RowGroup::Ai => "Ask",
    }
}

/// What kind of thing a local row is, in the person's words.
fn kind_of(action: &BarAction) -> Option<&'static str> {
    match action {
        BarAction::Go(target) => Some(match target {
            Target::Space(_) => "Space",
            Target::Tab(_) => "Tab",
            Target::Closed(_) => "Recently closed",
            Target::Folder(_) => "Tab group",
            Target::Host(_) => "Computer",
            Target::Layout(_) => "Layout",
            Target::RecentCommand(_) => "Recent command",
            Target::Directory { .. } => "Folder",
        }),
        BarAction::NewTab
        | BarAction::Run(_)
        | BarAction::Plugin { .. }
        | BarAction::Suggest(_)
        | BarAction::AskAgent(_)
        | BarAction::AskAbout(_) => None,
    }
}

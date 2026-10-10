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

use anyview_pane::{HitList, PaneCommand, PaneHandle};
use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::{
    GroupEntries, PaletteGroup, PaletteGroups, PaletteRow,
};
use ds::prelude::{AfterPick, CommandPalette, RowChord, TextLine};
use temor_core::bar::{BarAction, BarRow, RowGroup, Target};

use crate::bar::{FIND_PLACEHOLDER, Finding, Reply, action_of, entry_row, lift, reply};
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
    let mut finding = use_signal(Finding::default);
    let mut list = use_signal(HitList::default);
    // The viewer whose find is up; the bar putting it away is the bar going, however it goes.
    let searched = use_hook(|| Rc::new(Cell::new(None::<PaneHandle>)));
    use_drop({
        let searched = Rc::clone(&searched);
        move || {
            if let Some(handle) = searched.get() {
                handle.end_find();
            }
        }
    });
    let searching = handle.filter(|_| finding() == Finding::On);
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
            onclose: move |()| closed.bar_close(),
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

/// The rows as the palette's groups, in the order they were ranked: a run of rows from one rule
/// is one group, so the palette's numbering is the rows' own.
fn groups_of(rows: &[BarRow], hub: &HubRef) -> Vec<PaletteGroup<Pick>> {
    let mut groups: Vec<(RowGroup, Vec<PaletteRow<Pick>>)> = Vec::new();
    for row in rows {
        let line = PaletteRow {
            detail: kind_of(&row.action).map(TextLine::from),
            // A row that is also a pane action shows its chord on the highlighted row.
            chord: action_of(&row.action)
                .and_then(|action| hub.shortcut_of(action))
                .map(RowChord::on_selected)
                .unwrap_or_default(),
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

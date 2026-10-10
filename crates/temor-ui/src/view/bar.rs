//! The ⌘T command bar (PLAN §7): quire's `CommandPalette` over the rows temor-core ranked. The
//! palette draws and walks them; it never re-ranks. The hub keeps the query and the highlight
//! (quire's palette machine, `bar::BarModel`), so this view only reports what the person did.

use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::{PaletteGroup, PaletteGroups, PaletteRow};
use ds::prelude::{AfterPick, CommandPalette, TextLine};
use temor_core::bar::{BarAction, BarRow, RowGroup, Target};

use crate::hub::{BarData, Desk};

/// The bar, over the window.
#[component]
#[allow(non_snake_case)]
pub fn BarView(data: BarData, desk: Desk) -> Element {
    let (typed, selected, picked, closed) =
        (desk.clone(), desk.clone(), desk.clone(), desk.clone());
    rsx! {
        CommandPalette::<BarRow> {
            label: "Command bar",
            placeholder: "Search tabs, Spaces and commands",
            query: data.query.clone(),
            tokens: Vec::<String>::new(),
            groups: PaletteGroups(groups_of(&data.rows)),
            empty: "Nothing matches.",
            oninput: move |text: String| typed.bar_typed(text),
            on_select: Some(EventHandler::new(move |index: usize| selected.bar_select(index))),
            selected: Some(data.selected),
            onpick: move |row: BarRow| picked.bar_pick(&row.key),
            onclose: move |()| closed.bar_close(),
        }
    }
}

/// The rows as the palette's groups, in the order they were ranked: a run of rows from one rule
/// is one group, so the palette's numbering is the rows' own.
fn groups_of(rows: &[BarRow]) -> Vec<PaletteGroup<BarRow>> {
    let mut groups: Vec<(RowGroup, Vec<PaletteRow<BarRow>>)> = Vec::new();
    for row in rows {
        let line = PaletteRow {
            detail: kind_of(&row.action).map(TextLine::from),
            // The palette stays up until the bar's own step closes it, so the pick reaches the
            // palette machine once.
            after: AfterPick::KeepOpen,
            ..PaletteRow::new(row.clone(), row.title.clone())
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

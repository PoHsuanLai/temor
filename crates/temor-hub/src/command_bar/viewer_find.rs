//! Find in a viewer pane, from the ⌘T bar (PLAN §10): while a viewer has the keyboard the bar can
//! switch to a "find in this file" mode. What is typed goes to the pane's find, the matches come
//! back as an "In This File" group, and a picked match is run by the pane. ⌘F stays the
//! terminal's find. This holds the pure side: the entry row, the mode, how a pick is answered and
//! how the pane's rows become the bar's; `view::bar` draws them.

use anyview_pane::PaneCommand;
use ds::components::menus::palette::palette_group::PaletteRow;
use ds::prelude::{AfterPick, PaletteClose, TextLine};

/// What closing the bar does: Escape and a pick, or a click outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escape {
    /// Leave the file's find and show the bar's rows again; the next Escape closes the bar.
    StepBack,
    /// Close the bar, and the file's find with it.
    Close,
}

/// What the palette's close does while the field is in `finding`. Escape and a pick leave the
/// file's find first; a click outside the bar closes it whole, as it closes any floating card.
pub fn escape(finding: bool, how: PaletteClose) -> Escape {
    match (how, finding) {
        (PaletteClose::Outside, _) | (_, false) => Escape::Close,
        (_, true) => Escape::StepBack,
    }
}

/// What a picked match of the file's find asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    /// List every match, not the first few.
    ListAll,
    /// Go to the match: the pane runs it.
    Go,
}

/// What picking `command` of the "In This File" group asks for.
pub fn reply(command: &PaneCommand) -> Reply {
    match command {
        PaneCommand::ShowAllHits => Reply::ListAll,
        _ => Reply::Go,
    }
}

/// The placeholder of the field while it searches the file.
pub const PLACEHOLDER: &str = "Find in this file";

/// The row that switches the bar to searching the open file, when its name holds `query`.
pub fn entry_row<T>(value: T, query: &str) -> Option<PaletteRow<T>> {
    const TITLE: &str = "Find in This File";
    let wanted = query.trim().to_lowercase();
    TITLE.to_lowercase().contains(&wanted).then(|| PaletteRow {
        detail: Some(TextLine::from("Search the file that is open")),
        after: AfterPick::KeepOpen,
        ..PaletteRow::new(value, TITLE)
    })
}

/// The pane's rows as rows of the bar yielding what `wrap` makes of each command. They stay up
/// when picked: the bar's own step closes it, and a match picked leaves the find up.
pub fn lift<T>(
    rows: Vec<PaletteRow<PaneCommand>>,
    wrap: impl Fn(PaneCommand) -> T,
) -> Vec<PaletteRow<T>> {
    rows.into_iter()
        .map(|row| PaletteRow {
            value: wrap(row.value),
            title: row.title,
            detail: row.detail,
            leading: row.leading,
            accessory: row.accessory,
            chord: row.chord,
            availability: row.availability,
            action: row.action,
            after: AfterPick::KeepOpen,
            shape: row.shape,
        })
        .collect()
}

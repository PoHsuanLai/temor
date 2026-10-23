//! A pane's blocks as the list the window draws (PLAN: block list): a folder line where the
//! folder changes, then each block's head and the screen rows of its output, and the composer
//! last while nothing runs. Made from a [`Read`]; rows are screen rows (0 is the oldest kept
//! row), which a block's surface reads whole wherever they are, viewport or scrollback.

use std::ops::Range;

use crate::block::Outcome;
use crate::vocab::ids::BlockId;

use super::frame::{BlockRead, End, Read, command_rows};

/// What the list needs to know of a block that the terminal does not say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockFacts {
    /// The folder the command ran in, when known.
    pub folder: Option<String>,
    /// The person folded its output away.
    pub folded: bool,
    /// The person bookmarked it.
    pub bookmarked: bool,
}

/// One block in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBlock {
    pub id: BlockId,
    /// The command's first line.
    pub command: String,
    pub outcome: Outcome,
    pub took_ms: Option<u64>,
    /// The screen rows of its output; empty when it printed nothing.
    pub output: Range<u32>,
    pub folded: bool,
    pub picked: bool,
    pub bookmarked: bool,
}

impl ListBlock {
    /// How many output rows it shows: none while folded.
    pub fn shown_rows(&self) -> u32 {
        match self.folded {
            true => 0,
            false => self.output.len() as u32,
        }
    }
}

/// One entry of the list, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// The folder the blocks under it ran in, named where it changes.
    Section { folder: String, before: BlockId },
    /// A finished block: its output never changes again.
    Done(ListBlock),
    /// The running block: its output rows above `live_from` have left the viewport and no longer
    /// change; those from it on are the viewport's, drawn live.
    Running { block: ListBlock, live_from: u32 },
    /// Where the next command is typed: nothing runs. `rows` are the screen rows of the shell's
    /// own prompt, from its first row to the cursor's (empty when no prompt is marked).
    Composer { rows: Range<u32> },
}

/// What identifies an item across reads, for the list to keep its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKey {
    Section(BlockId),
    Block(BlockId),
    Composer,
}

impl Item {
    pub fn key(&self) -> ItemKey {
        match self {
            Item::Section { before, .. } => ItemKey::Section(*before),
            Item::Done(block) | Item::Running { block, .. } => ItemKey::Block(block.id),
            Item::Composer { .. } => ItemKey::Composer,
        }
    }
}

/// The first output row of a block: after the rows its command line takes.
fn output_start(read: &BlockRead) -> Option<u32> {
    let output = read.output?;
    let start = match read.input {
        Some(input) => {
            let (_, last) = command_rows(
                i64::from(input.row),
                Some((i64::from(output.row), output.col)),
                i64::from(input.row),
            );
            last + 1
        }
        // Without an input mark the output mark alone says where the command line ends.
        None => i64::from(output.row) + i64::from(output.col > 0),
    };
    u32::try_from(start).ok()
}

/// The screen rows of a block's output: to the row before its end mark when that mark is at a
/// line's start (the next prompt begins there), to the cursor's row while it runs. `None` for a
/// block the terminal no longer holds.
fn output_of(read: &BlockRead, cursor: Option<u32>, bottom: u32) -> Option<Range<u32>> {
    let start = output_start(read);
    let end = match read.end {
        End::At(end) => match end.col {
            0 => end.row,
            _ => end.row + 1,
        },
        End::Running => cursor.unwrap_or(bottom) + 1,
        End::Lost => return None,
    };
    let start = start.unwrap_or(end).min(end);
    Some(start..end)
}

/// The list for `read`: every block the terminal still holds whose command is known, oldest
/// first, with `facts` for each.
pub fn list_of(read: &Read, facts: impl Fn(BlockId) -> BlockFacts) -> Vec<Item> {
    let bottom = read.top.saturating_add(read.len.saturating_sub(1));
    let mut items = Vec::with_capacity(read.blocks.len() * 2 + 1);
    let mut folder: Option<String> = None;
    let mut running = false;
    for block in &read.blocks {
        let command = block.command.lines().next().unwrap_or("").trim();
        if command.is_empty() {
            continue;
        }
        let Some(output) = output_of(block, read.cursor, bottom) else {
            continue;
        };
        let fact = facts(block.id);
        if let Some(here) = fact.folder.filter(|f| !f.is_empty())
            && folder.as_ref() != Some(&here)
        {
            items.push(Item::Section {
                folder: here.clone(),
                before: block.id,
            });
            folder = Some(here);
        }
        let entry = ListBlock {
            id: block.id,
            command: command.to_owned(),
            outcome: block.outcome,
            took_ms: block.took_ms,
            output,
            folded: fact.folded,
            picked: block.picked,
            bookmarked: fact.bookmarked,
        };
        items.push(match block.end {
            End::Running => {
                running = true;
                let live_from = read.top.clamp(entry.output.start, entry.output.end);
                Item::Running {
                    block: entry,
                    live_from,
                }
            }
            _ => Item::Done(entry),
        });
    }
    if !running {
        let rows = match (&read.live, read.cursor) {
            (Some(live), Some(cursor)) if cursor >= live.prompt.row => live.prompt.row..cursor + 1,
            _ => 0..0,
        };
        items.push(Item::Composer { rows });
    }
    items
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;

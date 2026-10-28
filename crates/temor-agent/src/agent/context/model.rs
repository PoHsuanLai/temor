//! A card, its chips, and the record of what was sent.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};
use temor_core::vocab::ids::Bytes;

/// Where the request came from. Each entry point has its own fixed defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum EntryPoint {
    /// The Explain or Fix chip on a failed block.
    #[word(label = "Explain or fix")]
    FailedBlock,
    /// The ⌘T row "Suggest a command for: …".
    #[word(label = "Suggest a command")]
    SuggestCommand,
    /// The ⌘T row "Ask about" a tab.
    #[word(label = "Ask about a tab")]
    AskAboutTab,
}

/// What a chip holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum ChipKind {
    /// What the person typed or asked.
    #[word(label = "Your request")]
    Request,
    /// The working folder.
    #[word(label = "Folder")]
    Cwd,
    /// Which shell.
    #[word(label = "Shell")]
    Shell,
    /// The last block's command line.
    #[word(label = "Last command")]
    Command,
    /// The last lines of that block's output.
    #[word(label = "Command output")]
    Output,
    /// The text selected.
    #[word(label = "Selection")]
    Selection,
    /// A file name (names only, never contents).
    #[word(label = "File name")]
    FileName,
}

/// A chip's number within its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChipId(pub u32);

/// One thing that will be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// Its number.
    pub id: ChipId,
    /// What it is.
    pub kind: ChipKind,
    /// The short text the chip shows (a folder name, the first words of the output).
    pub label: String,
    /// Exactly what goes to the model.
    pub text: String,
    /// Whether the person may remove it. The request itself may not: without it there is
    /// nothing to send.
    pub removable: bool,
}

impl Chip {
    /// How many bytes it adds.
    pub fn size(&self) -> Bytes {
        Bytes(self.text.len() as u64)
    }
}

/// The card shown before a model call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCard {
    /// Where the request came from.
    pub entry: EntryPoint,
    /// Chips that will be sent, in order.
    pub chips: Vec<Chip>,
    /// Chips the person took off, kept so "Add back" works.
    pub removed: Vec<Chip>,
}

impl ContextCard {
    /// The bytes that would be sent now.
    pub fn total(&self) -> Bytes {
        Bytes(self.chips.iter().map(|c| c.size().0).sum())
    }
}

/// One chip as sent. The log record keeps the text so "what did the model see?" has an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentChip {
    /// What it was.
    pub kind: ChipKind,
    /// What the chip showed.
    pub label: String,
    /// Its size in bytes.
    pub size: Bytes,
    /// What was sent.
    pub text: String,
}

/// What was sent for one request; stored with the request's log record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentList {
    /// Where the request came from.
    pub entry: EntryPoint,
    /// The chips, in the order sent.
    pub chips: Vec<SentChip>,
    /// The total size.
    pub total: Bytes,
}

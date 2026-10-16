//! What temor and a plugin say to each other. The shapes follow bayonet's `docs/capabilities.md`;
//! the payloads are temor's. `open.provider` rows are the launcher provider shape (sill's), so one
//! plugin can feed both.

use serde::{Deserialize, Serialize};

use crate::capability::Cap;

/// A block to draw: the command that ran and what it printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LensBlock {
    /// The command as typed.
    pub command: String,
    /// What it printed, as plain text, cut to the most a request carries.
    pub output: String,
    /// The folder it ran in; empty when unknown.
    pub folder: String,
    /// The room the picture has, in pixels.
    pub width: u32,
}

/// What was chosen on a row, as a launcher's provider receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Choice {
    /// Enter or a click.
    Primary,
    /// One of the row's own actions, by id.
    Action(String),
}

/// A row's icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Icon {
    /// No icon.
    None,
    /// A file.
    File(String),
    /// A named glyph.
    Glyph(String),
}

/// A secondary action of a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// The id handed back in [`Choice::Action`].
    pub id: String,
    /// What the menu says.
    pub label: String,
}

/// One result row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Identity within the plugin, stable across keystrokes.
    pub id: String,
    /// The row's title.
    pub title: String,
    /// The faint second line; empty for none.
    pub subtitle: String,
    /// Where the icon comes from.
    pub icon: Icon,
    /// The plugin's rank for the row, in thousandths; higher sorts first.
    pub score: u32,
    /// Secondary actions, in menu order.
    pub actions: Vec<Action>,
}

/// Some results for one query generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    /// The keystroke the batch answers; a newer one makes it stale.
    pub generation: u64,
    /// The rows.
    pub items: Vec<Item>,
}

/// What choosing a row asks temor to do: a closed vocabulary temor matches exhaustively. A plugin
/// can ask for no more than these three.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Activation {
    /// Put text on the clipboard.
    Copy {
        /// The text.
        text: String,
    },
    /// Open a file or an address.
    Open {
        /// What to open.
        target: OpenTarget,
    },
    /// Nothing to do.
    Nothing,
}

/// What an [`Activation::Open`] opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum OpenTarget {
    /// A file or folder, by absolute path.
    File(String),
    /// An address.
    Url(String),
}

/// One fact about a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// The label, short (`cluster`).
    pub label: String,
    /// The value, formatted for a person (`prod`).
    pub value: String,
}

/// What an agent is doing, as an adapter reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Nothing under way.
    Idle,
    /// Working.
    Working,
    /// Waiting for the person to allow something.
    WaitingPermission,
    /// Waiting for an answer.
    WaitingQuestion,
    /// Waiting for the person to sign in.
    WaitingSignIn,
    /// Finished.
    Done,
    /// Failed.
    Failed,
}

/// What temor says to a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Request {
    /// `block.lens`: draw this block.
    Lens(LensBlock),
    /// `open.provider`: results for the text typed (the trigger already taken off).
    Query {
        /// The text.
        text: String,
        /// The keystroke.
        generation: u64,
    },
    /// `open.provider`: what choosing `choice` on `item` does.
    Activate {
        /// The row.
        item: Item,
        /// What was chosen on it.
        choice: Choice,
    },
    /// `agent.adapter`: report on the agent running in `folder` until told to stop.
    Adapt {
        /// The agent, as the manifest names it.
        agent: String,
        /// The folder its pane is in.
        folder: String,
    },
    /// `row.facts`: facts about a row of a table.
    Facts {
        /// The table (`folder`).
        table: String,
        /// The row's key (the folder's path).
        key: String,
    },
    /// Stop what is running.
    Cancel,
}

/// What a plugin says to temor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Message {
    /// The first message.
    Hello {
        /// The protocol version.
        protocol: u32,
        /// The capabilities it answers.
        provides: Vec<Cap>,
    },
    /// `block.lens`: the drawing is the frame's payload, RGBA8.
    Drawn {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// `open.provider`: some results.
    Batch(Batch),
    /// `open.provider`: the query is answered.
    Complete,
    /// `open.provider`: the answer to `Activate`.
    Activation(Activation),
    /// `agent.adapter`: the agent's state changed.
    Event {
        /// The state.
        phase: Phase,
        /// What a person can read; empty for none.
        text: String,
    },
    /// `row.facts`: the facts.
    Facts(Vec<Fact>),
    /// `agent.adapter`: there is nothing more to report.
    Finished,
    /// The request failed or was cancelled.
    Failed {
        /// What a person can read.
        message: String,
    },
}

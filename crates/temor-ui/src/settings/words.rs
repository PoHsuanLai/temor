//! The words temor's settings are stored as. Each closed set is a `Word` enum, so the schema
//! names every value and a Settings page picks its control by them (a pair with `off` is a
//! switch). They convert to and from `temor-pane`'s own typed values.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};
use temor_pane::settings::{CursorBlinking, CursorStyle, PagerChoice, StartIn};

/// `cursor.style`: the cursor's shape, until a program asks for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

/// `cursor.blink`: whether the cursor blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Blinking {
    #[default]
    On,
    Off,
}

impl From<CursorShape> for CursorStyle {
    fn from(shape: CursorShape) -> Self {
        match shape {
            CursorShape::Block => Self::Block,
            CursorShape::Bar => Self::Bar,
            CursorShape::Underline => Self::Underline,
        }
    }
}

impl From<CursorStyle> for CursorShape {
    fn from(style: CursorStyle) -> Self {
        match style {
            CursorStyle::Block => Self::Block,
            CursorStyle::Bar => Self::Bar,
            CursorStyle::Underline => Self::Underline,
        }
    }
}

impl From<Blinking> for CursorBlinking {
    fn from(blinking: Blinking) -> Self {
        match blinking {
            Blinking::On => Self::On,
            Blinking::Off => Self::Off,
        }
    }
}

impl From<CursorBlinking> for Blinking {
    fn from(blinking: CursorBlinking) -> Self {
        match blinking {
            CursorBlinking::On => Self::On,
            CursorBlinking::Off => Self::Off,
        }
    }
}

/// `shell.start_in`: the folder a new tab or a split opens in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum StartFolder {
    /// A new tab in the home folder, a split in the folder it came from.
    #[default]
    #[word(label = "Home for a tab, the same folder for a split")]
    Usual,
    #[word(label = "The current folder")]
    Current,
    #[word(label = "The home folder")]
    Home,
}

impl From<StartFolder> for StartIn {
    fn from(folder: StartFolder) -> Self {
        match folder {
            StartFolder::Usual => Self::Usual,
            StartFolder::Current => Self::Current,
            StartFolder::Home => Self::Home,
        }
    }
}

impl From<StartIn> for StartFolder {
    fn from(start: StartIn) -> Self {
        match start {
            StartIn::Usual => Self::Usual,
            StartIn::Current => Self::Current,
            StartIn::Home => Self::Home,
        }
    }
}

/// `shell.pager`: where the output of a command that pages goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum PagerWay {
    /// Printed into the command's block.
    #[default]
    #[word(label = "In the command's block")]
    Block,
    #[word(label = "In the program's own pager")]
    System,
}

/// Whether quitting temor ends its terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum KeepWay {
    /// Quitting ends them.
    #[default]
    #[word(label = "Quitting ends them")]
    Off,
    /// A background service keeps them; the next start finds them.
    #[word(label = "Keep them running")]
    On,
}

impl From<KeepWay> for bool {
    fn from(way: KeepWay) -> Self {
        way == KeepWay::On
    }
}

impl From<bool> for KeepWay {
    fn from(keep: bool) -> Self {
        match keep {
            true => Self::On,
            false => Self::Off,
        }
    }
}

impl From<PagerWay> for PagerChoice {
    fn from(way: PagerWay) -> Self {
        match way {
            PagerWay::Block => Self::Block,
            PagerWay::System => Self::System,
        }
    }
}

impl From<PagerChoice> for PagerWay {
    fn from(choice: PagerChoice) -> Self {
        match choice {
            PagerChoice::Block => Self::Block,
            PagerChoice::System => Self::System,
        }
    }
}

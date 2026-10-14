//! The words temor's settings are stored as. Each closed set is a `Word` enum, so the schema
//! names every value and a Settings page picks its control by them (a pair with `off` is a
//! switch). They convert to and from `temor-pane`'s own typed values.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};
use temor_pane::settings::{CursorBlinking, CursorStyle};

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

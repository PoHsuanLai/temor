//! `temor/settings.toml`: the settings structs, one table each, deriving `SettingsSchema` so a
//! Settings page can draw them from the schema `temor --write-schema <dir>` writes. The tables
//! and keys are the ones `terminal.conf` had (`font.family`, `cursor.style`, ...), now as TOML.
//!
//! Two pages' worth of keys, told apart by section: General (Shell, Notices, Shell offer) and
//! Appearance (Text, Cursor, History; `settings_window::page_of` is the one place that says). The theme is quire's (`appearance.toml`) and is not
//! here.

use ds_settings::schema::Page;
use ds_settings::{FileName, Format, SettingsDoc};
use serde::{Deserialize, Serialize};

use super::schema::APP;
use super::values::{FontSize, NotifySeconds, ScrollbackSize};
use super::words::{Blinking, CursorShape, StartFolder};

/// The page temor's keys are on in the Settings app: temor's own.
fn page() -> Page {
    Page::App(APP.0.to_owned())
}

/// `font.*`: the terminal's text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "font", page = page())]
pub struct FontSettings {
    #[settings(
        label = "Font",
        help = "The family of the terminal's text. A name typed into settings.toml works too, such as JetBrains Mono.",
        section = "Text"
    )]
    pub family: String,
    #[settings(
        label = "Font size",
        help = "How big the terminal's text is.",
        section = "Text",
        range = "6..=72",
        unit = "points"
    )]
    pub size: FontSize,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            family: "system".to_owned(),
            size: FontSize::default(),
        }
    }
}

/// `cursor.*`: the terminal's cursor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "cursor", page = page())]
pub struct CursorSettings {
    #[settings(
        label = "Cursor shape",
        help = "A program can still ask for another shape.",
        section = "Cursor"
    )]
    pub style: CursorShape,
    #[settings(
        label = "Blinking cursor",
        help = "Off keeps the cursor steady whatever a program asks.",
        section = "Cursor"
    )]
    pub blink: Blinking,
}

/// `scrollback.*`: the history kept above the screen.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "scrollback", page = page())]
pub struct ScrollbackSettings {
    #[settings(
        label = "History kept",
        help = "How many lines of output each pane keeps above the screen. Applies to new panes.",
        section = "History",
        range = "0..=1000000",
        unit = "lines"
    )]
    pub lines: ScrollbackSize,
}

/// `shell.*`: what a new pane runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "shell", page = page())]
pub struct ShellSettings {
    #[settings(
        label = "Shell",
        help = "login runs your account's own shell; or name a program or its path. Applies to new panes.",
        section = "Shell"
    )]
    pub program: String,
    #[settings(
        label = "New tabs start in",
        help = "Where a new tab or a split opens: the folder of the pane in front, or the home folder. The usual way is the home folder for a tab and the same folder for a split.",
        section = "Shell"
    )]
    pub start_in: StartFolder,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            program: "login".to_owned(),
            start_in: StartFolder::default(),
        }
    }
}

/// `notify.*`: telling the person a command finished.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "notify", page = page())]
pub struct NotifySettings {
    #[settings(
        label = "Tell me when a command finishes",
        help = "After a command has run this long while its pane is out of sight. 0 turns the notices off.",
        section = "Notices",
        range = "0..=86400",
        unit = "seconds"
    )]
    pub after: NotifySeconds,
}

/// `offer.*`: the commands temor offers to set up blocks in (ssh, su, ...).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ds_settings::SettingsSchema)]
#[serde(default)]
#[settings(file = "temor/settings.toml", domain = "offer", page = page())]
pub struct OfferSettings {
    #[settings(
        label = "Also offer for",
        help = "Commands to offer blocks for, as the first words of a command line, such as kubectl debug -it.",
        section = "Shell offer"
    )]
    pub add: Vec<String>,
    #[settings(
        label = "Never offer for",
        help = "Commands to leave alone, including the ones temor offers for by itself.",
        section = "Shell offer"
    )]
    pub remove: Vec<String>,
}

/// `temor/settings.toml`, whole.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalDoc {
    pub font: FontSettings,
    pub cursor: CursorSettings,
    pub scrollback: ScrollbackSettings,
    pub shell: ShellSettings,
    pub notify: NotifySettings,
    pub offer: OfferSettings,
}

impl SettingsDoc for TerminalDoc {
    const FILE: FileName = FileName("settings.toml");
    const FORMAT: Format = Format::Toml;
}

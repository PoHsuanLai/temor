//! Settings: a window of its own, as a Mac app's settings are and as mailo's are, opened with ⌘,
//! the command bar or its page rows, and raised when it is already open. A sidebar of four pages,
//! each a `Form` of titled groups of rows.
//!
//! General and Appearance draw temor's settings schema through quire's `SettingsPage`, plus rows
//! of the window's own for the text, numbers and lists the page only shows (`general`,
//! `own_rows`, split by `pages`); every change is saved through the store (`edit`) and the open
//! panes follow the file. Keyboard is one shortcut recorder for each action (`keyboard`,
//! `keys_edit`), saved in `keys.conf`. Spaces shows each Space's folder, profile and routes
//! (`spaces`), changes going to the main window through the board (`board`), which holds the
//! kit's handle.
//!
//! The window is quire's (`ds_blitz::open_window`), with the root contexts the first window is
//! given (`open`).

mod board;
mod edit;
mod font_row;
mod general;
mod keyboard;
mod keys_edit;
mod open;
mod own_rows;
mod pages;
mod root;
mod row_parts;
mod space_edit;
mod space_fields;
mod space_profile;
mod space_secrets;
mod spaces;
mod style;
#[cfg(test)]
mod tests;
mod view;

pub use board::{SpaceBoard, SpaceEdit, SpaceRow};
pub use edit::{Edited, applied, save_edit};
pub use keys_edit::{KeyChange, change_keys, load_view};
pub use open::{
    OpenSettings, SettingsAsked, SettingsOpened, SettingsWindows, TITLE, open, open_at,
    use_settings_opened,
};
pub use pages::page_of;
pub use root::{settings_root, settings_window};
pub use space_edit::{SpaceChange, route_warning};
pub use style::SETTINGS_CSS;

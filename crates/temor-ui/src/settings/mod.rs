//! temor's settings on ds-settings (PLAN §14): the schema structs, the file they live in
//! (`temor/settings.toml`), the keys in their own file (`temor/keys.conf`), the one-time
//! conversion of the old `terminal.conf`, and the live follow of both files.
//!
//! The layers hold: `temor-pane` keeps `TerminalSettings` and its typed values and never sees
//! ds-settings; this module reads the file into [`TerminalDoc`] and converts it into them.
//!
//! The theme stays where it is, in quire's `appearance.toml` (`view::theme`).

mod convert;
mod doc;
mod keys_file;
mod live;
mod load;
mod migrate;
mod schema;
#[cfg(test)]
mod tests;
mod values;
mod words;

pub use convert::from_terminal;
pub use doc::{
    CursorSettings, FontSettings, NotifySettings, OfferSettings, ScrollbackSettings, ShellSettings,
    TerminalDoc,
};
pub use keys_file::{overrides_of, read_keys_text, read_overrides, write_keys};
pub use live::{LiveChange, live_changes};
pub use load::{Loaded, load_settings};
pub use migrate::{Converted, convert_terminal_conf, migrate_terminal_conf};
pub use schema::{APP, root_for, schema, write_schema};
pub use values::{FontSize, NotifySeconds, ScrollbackSize};
pub use words::{Blinking, CursorShape};

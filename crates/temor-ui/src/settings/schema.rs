//! temor's schema: every table's keys under app `temor`, the file a Settings page draws from
//! (`temor --write-schema <dir>`), and where the file lives.

use std::path::Path;

use ds_settings::schema::{AppId, FilePath, Schema, SettingsSchema};
use ds_settings::{AppName, ConfigRoot};

use super::doc::{
    CursorSettings, FontSettings, NotifySettings, OfferSettings, ScrollbackSettings, ShellSettings,
};

/// The directory under the config root, and the schema's app id.
pub const APP: AppName = AppName("temor");

/// The settings file, relative to the config root, as the schema names it.
const FILE: &str = "temor/settings.toml";

/// The one `temor.settings.toml` the package installs: every table's keys under app `temor`.
pub fn schema() -> Schema {
    let key = [
        FontSettings::schema(),
        CursorSettings::schema(),
        ScrollbackSettings::schema(),
        ShellSettings::schema(),
        NotifySettings::schema(),
        OfferSettings::schema(),
    ]
    .into_iter()
    .flat_map(|schema| schema.key)
    .collect();
    Schema {
        app: AppId(APP.0.to_owned()),
        file: FilePath(FILE.to_owned()),
        version: 1,
        key,
        // temor keeps no tables of its own data in the file.
        foreign: Default::default(),
        // Every key sits on one of the built-in pages.
        pages: Vec::new(),
    }
}

/// The config root a temor config directory stands for: `~/.config/temor` is `~/.config`, the
/// root it is temor's directory in. Any other directory (a test's scratch) is a root of its own,
/// so `settings.toml` lands in its `temor/`.
pub fn root_for(config_dir: &Path) -> ConfigRoot {
    match (config_dir.file_name(), config_dir.parent()) {
        (Some(name), Some(parent)) if name == APP.0 => ConfigRoot::Scratch(parent.to_path_buf()),
        _ => ConfigRoot::Scratch(config_dir.to_path_buf()),
    }
}

/// `temor --write-schema <dir>`: write `temor.settings.toml` into `dir`, and say where.
pub fn write_schema(dir: &Path) -> Result<String, String> {
    let path = schema().write_to(dir).map_err(|e| e.to_string())?;
    Ok(format!("wrote {}\n", path.display()))
}

//! The app's reads from outside: the process environment, the settings file. Everything here is
//! read once, at start, and handed to the rest of the app as values.

mod ambient;
mod settings_file;

pub use ambient::Ambient;
pub use settings_file::load_settings;

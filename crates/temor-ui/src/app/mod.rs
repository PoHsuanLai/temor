//! The app's entry: what the window is given at start, and how it is launched.

mod shell;

pub use shell::shell_pane;

use crate::view::{App, Launch};

/// The desktop's identity for this app (its desktop file is `dist/<id>.desktop`).
pub const APP_ID: &str = "org.quire.Temor";

/// Opens temor's window and runs until it closes. A window that cannot open prints why and exits
/// the process with status 1.
pub fn run(launch: Launch) {
    let config = ds_blitz::AppConfig::new(
        "temor",
        ds_blitz::WindowSize::new(960, 600).with_least(480, 320),
    )
    .with_app_id(ds_blitz::AppId(APP_ID.to_owned()))
    .with_context(launch);
    if let Err(err) = ds_blitz::launch(App, config) {
        eprintln!("temor: {err}");
        std::process::exit(1);
    }
}

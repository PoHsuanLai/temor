//! The app's entry: what the window is given at start, and how it is launched.

pub mod desktop;
mod shell;

pub use shell::shell_pane;

use crate::view::{App, Launch};

pub use desktop::Desktop;

/// The desktop's identity for this app (its desktop file is `dist/<id>.desktop`).
pub const APP_ID: &str = "org.quire.Temor";

/// Opens temor's window and runs until it closes. Fails when the window cannot open.
pub fn run(launch: Launch) -> Result<(), ds_blitz::LaunchError> {
    run_with(launch, Desktop::none())
}

/// [`run`] with the desktop services `desktop` (see [`desktop::join`]).
pub fn run_with(launch: Launch, desktop: Desktop) -> Result<(), ds_blitz::LaunchError> {
    let config = ds_blitz::AppConfig::new(
        "temor",
        ds_blitz::WindowSize::new(960, 600).with_least(480, 320),
    )
    .with_app_id(ds_blitz::AppId(APP_ID.to_owned()))
    .with_context(launch)
    .with_context(desktop);
    ds_blitz::launch(App, config)
}

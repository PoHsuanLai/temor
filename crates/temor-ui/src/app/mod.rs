//! The app's entry: what the window is given at start, and how it is launched.

pub mod desktop;
mod icon;
mod shell;

pub use shell::{shell_pane, shell_pane_with_socket};

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
    .with_context(desktop)
    // temor binds the page zoom itself, and Alt with D, H or T is the shell's.
    .with_shell_keys(ds_blitz::ShellKeys::App);
    // X11 and Windows show the window's own icon; elsewhere the desktop entry gives it.
    let config = match icon::window_icon() {
        Some(icon) => config.with_icon(icon),
        None => config,
    };
    ds_blitz::launch(App, config)
}

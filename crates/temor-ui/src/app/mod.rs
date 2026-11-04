//! The app's entry: what the window is given at start, and how it is launched.

mod icon;
mod shell;

pub use shell::{shell_pane, shell_pane_with_socket};

use temor_ui_base::launch::Launch;
use temor_views::App;

pub use temor_ui_base::desktop::{self, Desktop};

/// The desktop's identity for this app (its desktop file is `dist/<id>.desktop`).
pub const APP_ID: &str = "org.quire.Temor";

/// Opens temor's window and runs until it closes. Fails when the window cannot open.
pub fn run(launch: Launch) -> Result<(), ds_blitz::LaunchError> {
    run_with(launch, Desktop::none())
}

/// [`run`] with the desktop services `desktop` (see [`desktop::join`]).
pub fn run_with(launch: Launch, desktop: Desktop) -> Result<(), ds_blitz::LaunchError> {
    // One for the process: every window reads through it.
    let client = temor_views::live::open_client(launch.state_dir.as_deref());
    let config = ds_blitz::AppConfig::new(
        "temor",
        ds_blitz::WindowSize::new(960, 600).with_least(480, 320),
    )
    .with_app_id(ds_blitz::AppId(APP_ID.to_owned()))
    .with_context(launch)
    .with_context(desktop)
    .with_context(client)
    // Both windows hold these: where Settings is asked to be, and the Spaces between them.
    .with_context(temor_hub::settings_link::SettingsAsked::default())
    .with_context(temor_hub::settings_link::SettingsWindows(
        std::sync::Arc::new(temor_views::settings_window::QuireSettings),
    ))
    .with_context(temor_hub::settings_link::board::SpaceBoard::default())
    // temor binds the page zoom itself, and Alt with D, H or T is the shell's.
    .with_shell_keys(ds_blitz::ShellKeys::App);
    // X11 and Windows show the window's own icon; elsewhere the desktop entry gives it.
    let config = match icon::window_icon() {
        Some(icon) => config.with_icon(icon),
        None => config,
    };
    ds_blitz::launch(App, config)
}

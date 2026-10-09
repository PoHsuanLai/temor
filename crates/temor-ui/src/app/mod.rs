//! The app's entry: what the window is given at start, and how it is launched.

mod shell;

pub use shell::shell_pane;

use crate::view::{App, Launch};

/// The desktop's identity for this app (its desktop file is `dist/<id>.desktop`).
pub const APP_ID: &str = "org.quire.Temor";

/// Opens temor's window and runs until it closes.
pub fn run(launch: Launch) {
    ds_blitz::launch(
        App,
        ds_blitz::AppConfig::new(
            "temor",
            ds_blitz::WindowSize::new(960, 600).with_least(480, 320),
        )
        .with_app_id(ds_blitz::AppId(APP_ID.to_owned()))
        .with_context(launch),
    );
}

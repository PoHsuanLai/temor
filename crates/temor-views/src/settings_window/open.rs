//! Opening the Settings window, or raising it when it is open (as mailo's does): ⌘, the command
//! bar and its page rows all come here. The window is quire's (`ds_blitz::open_window`), which
//! gives it the same root contexts as the first window. Where it is asked to be is put in
//! [`SettingsAsked`], which every window shares, and an open Settings window follows it.
//!
//! The hub asks through the [`SettingsWindows`] the window was given (`temor_hub::settings_link`):
//! the launched window is given [`QuireSettings`], a test a recorder, since quire's harness has
//! no event loop to open a window on.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds_blitz::{WindowHandle, WindowLife, WindowSize, WindowSpec};
use temor_panes::pane::keys::SettingsPane;

use temor_hub::settings_link::{OpenSettings, SettingsAsked};

use super::root::settings_window;

/// What the window and its command are called.
pub const TITLE: &str = "Settings";

/// The size the window opens at, in logical pixels.
const SIZE: (u32, u32) = (780, 620);

/// The Settings window this window opened, kept so that opening it again raises it.
#[derive(Clone, Default)]
pub struct SettingsOpened(Rc<RefCell<Option<WindowHandle>>>);

impl std::fmt::Debug for SettingsOpened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SettingsOpened")
    }
}

/// Keep the Settings window this one opens. Called once, from `App`.
pub fn use_settings_opened() {
    use_context_provider(SettingsOpened::default);
}

/// Opening the Settings window through quire's event loop: what the launched window is given as
/// its [`SettingsWindows`]. Turns it to `at` through [`SettingsAsked`], then opens or raises it.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuireSettings;

impl OpenSettings for QuireSettings {
    fn open(&self, at: Option<SettingsPane>) {
        if let (Some(at), Some(asked)) = (at, try_consume_context::<SettingsAsked>()) {
            asked.ask(at);
        }
        quire();
    }
}

/// Whether a window opened earlier is one to raise rather than open again.
pub(super) fn raise(life: WindowLife) -> bool {
    matches!(life, WindowLife::Opening | WindowLife::Open)
}

/// Ask quire's event loop for the window, or raise the one already open.
fn quire() {
    let opened = try_consume_context::<SettingsOpened>().unwrap_or_default();
    let existing = opened.0.borrow().clone();
    if let Some(handle) = existing.filter(|handle| raise(handle.life())) {
        handle.focus();
        return;
    }
    match ds_blitz::open_window(
        WindowSpec::new(TITLE, WindowSize::new(SIZE.0, SIZE.1)),
        settings_window,
    ) {
        Ok(handle) => {
            *opened.0.borrow_mut() = Some(handle);
        }
        Err(why) => report_failure(&why.to_string()),
    }
}

/// A window that could not open has nowhere to say so but the main window's message bar, which
/// this module cannot reach from here; the person sees nothing open, and the reason goes to the
/// error stream.
fn report_failure(why: &str) {
    eprintln!("Could not open Settings: {why}");
}

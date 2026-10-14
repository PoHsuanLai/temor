//! Opening the Settings window, or raising it when it is open (as mailo's does): ⌘, the command
//! bar and its page rows all come here. The window is quire's (`ds_blitz::open_window`), which
//! gives it the same root contexts as the first window. Where it is asked to be is put in
//! [`SettingsAsked`], which every window shares, and an open Settings window follows it.
//!
//! The call goes through [`SettingsWindows`] when the window was given one, which is how a test
//! sees what would open: quire's harness has no event loop to open a window on.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;
use ds_blitz::{WindowHandle, WindowLife, WindowSize, WindowSpec};
use temor_pane::keys::SettingsPane;
use tokio::sync::watch;

use super::root::settings_window;

/// What the window and its command are called.
pub const TITLE: &str = "Settings";

/// The size the window opens at, in logical pixels.
const SIZE: (u32, u32) = (780, 620);

/// Whatever opens the Settings window: quire's event loop in the launched window, a recorder in
/// a test.
pub trait OpenSettings: Send + Sync + 'static {
    /// Open the Settings window, or raise it when it is open, turned to `at` when it is asked
    /// for and where it is otherwise.
    fn open(&self, at: Option<SettingsPane>);
}

/// Where the Settings window is next, as a root context every window shares: the main window
/// puts where it asks for here, a Settings window opening starts there, and an open one turns to
/// it. Without one (a test's single window) the window opens on General.
#[derive(Clone)]
pub struct SettingsAsked(Arc<watch::Sender<SettingsPane>>);

impl Default for SettingsAsked {
    fn default() -> Self {
        SettingsAsked(Arc::new(watch::channel(SettingsPane::default()).0))
    }
}

impl std::fmt::Debug for SettingsAsked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SettingsAsked")
            .field(&*self.0.borrow())
            .finish()
    }
}

impl SettingsAsked {
    /// Where it was asked to be last.
    pub fn at(&self) -> SettingsPane {
        *self.0.borrow()
    }

    /// Ask for `at`. Sent even when it is where it was asked to be last: the window may have been
    /// turned elsewhere since.
    pub fn ask(&self, at: SettingsPane) {
        self.0.send_replace(at);
    }

    /// What a Settings window hears each ask through.
    pub fn heard(&self) -> watch::Receiver<SettingsPane> {
        self.0.subscribe()
    }
}

/// Where ⌘, goes, as a root context. Without one it is quire's `open_window`.
#[derive(Clone)]
pub struct SettingsWindows(pub Arc<dyn OpenSettings>);

impl std::fmt::Debug for SettingsWindows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SettingsWindows")
    }
}

/// The Settings window this window opened, kept so that opening it again raises it.
#[derive(Clone, Default)]
pub struct SettingsOpened(Rc<RefCell<Option<WindowHandle>>>);

/// Keep the Settings window this one opens. Called once, from `App`.
pub fn use_settings_opened() {
    use_context_provider(SettingsOpened::default);
}

/// Open Settings, or raise it on the page it shows.
pub fn open() {
    ask(None);
}

/// Open Settings on `page`, or raise it and turn it to `page`.
pub fn open_at(page: SettingsPane) {
    ask(Some(page));
}

fn ask(at: Option<SettingsPane>) {
    match try_consume_context::<SettingsWindows>() {
        Some(windows) => windows.0.open(at),
        None => {
            if let (Some(at), Some(asked)) = (at, try_consume_context::<SettingsAsked>()) {
                asked.ask(at);
            }
            quire();
        }
    }
}

/// Whether a window opened earlier is one to raise rather than open again.
fn raise(life: WindowLife) -> bool {
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

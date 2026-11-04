//! What the main window and the Settings window share: the Spaces they both show ([`board`]),
//! the edits the Settings window makes to one ([`space_edit`]), and how the hub asks for the
//! Settings window (⌘, the command bar and its page rows) without naming the window that draws it.

pub mod board;
pub mod space_edit;

use std::sync::Arc;

use dioxus::prelude::*;
use temor_panes::pane::keys::SettingsPane;
use tokio::sync::watch;

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

/// Open Settings, or raise it on the page it shows. Nothing opens in a window given no
/// [`SettingsWindows`].
pub fn open() {
    ask(None);
}

/// Open Settings on `page`, or raise it and turn it to `page`.
pub fn open_at(page: SettingsPane) {
    ask(Some(page));
}

fn ask(at: Option<SettingsPane>) {
    if let Some(windows) = try_consume_context::<SettingsWindows>() {
        windows.0.open(at);
    }
}

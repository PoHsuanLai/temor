//! The root of the Settings window. quire gives this VirtualDom the same root contexts as the
//! first window (`Launch`, `Desktop`, the board); what the window keeps for itself is made here
//! again: its look, which page is shown and the settings it draws. It opens on the page the
//! main window asked for last ([`SettingsAsked`]) and turns to each page asked for while it is
//! open.

use std::sync::Arc;

use dioxus::prelude::*;
use ds::base::spawner::Spawner;
use ds::prelude::*;
use ds_blitz::TokioSpawner;
use ds_settings::{AppearanceFile, use_environment};
use temor_pane::keys::SettingsPane;

use super::edit::current;
use super::open::SettingsAsked;
use super::style::SETTINGS_CSS;
use super::view::SettingsView;
use crate::edge::read_theme_choice;
use crate::settings::TerminalDoc;
use crate::view::launch::Launch;
use crate::view::theme::dark_by_default;

/// Whether the window follows `settings.toml` while it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follow {
    /// A change made elsewhere (another window, an editor) is shown at once.
    File,
    /// The window shows the file as it was read: a test's.
    Fixed,
}

/// The window as quire opens it.
pub fn settings_window() -> Element {
    rsx! { SettingsShell { follow: Follow::File } }
}

/// The window as a test drives it: without the watch on the settings folder.
pub fn settings_root() -> Element {
    rsx! { SettingsShell { follow: Follow::Fixed } }
}

#[component]
#[allow(non_snake_case)]
fn SettingsShell(follow: Follow) -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let spawner = use_hook(|| Arc::new(TokioSpawner::current()) as Arc<dyn Spawner>);
    // The person's look and the desktop's preferences, as the first window follows them.
    let stored = use_environment(
        launch.appearance.store.clone(),
        launch.appearance.system.clone(),
        spawner.clone(),
    );
    let choice = use_memo({
        let store = launch.appearance.store.clone();
        move || {
            let _ = stored();
            read_theme_choice(store.path::<AppearanceFile>().as_deref())
        }
    });
    let env = use_memo(move || dark_by_default(stored(), choice()));

    let asked = try_consume_context::<SettingsAsked>();
    let first = asked.as_ref().map(SettingsAsked::at).unwrap_or_default();
    let mut page = use_signal(move || first);
    use_future(move || {
        let asked = asked.clone();
        async move {
            let Some(asked) = asked else { return };
            let mut heard = asked.heard();
            heard.mark_unchanged();
            while heard.changed().await.is_ok() {
                let at: SettingsPane = *heard.borrow_and_update();
                page.set(at);
            }
        }
    });

    let mut doc = use_signal({
        let ambient = launch.ambient.clone();
        move || current(&ambient)
    });
    use_context_provider(|| doc);
    use_future({
        let store = launch.ambient.settings_store();
        move || {
            let (store, spawner) = (store.clone(), spawner.clone());
            async move {
                if follow == Follow::Fixed {
                    return;
                }
                let mut watch = store.watch::<TerminalDoc>(&*spawner);
                while let Some(loaded) = watch.changed().await {
                    if *doc.peek() != loaded.value {
                        doc.set(loaded.value);
                    }
                }
            }
        }
    });

    let now = env();
    rsx! {
        Ds {
            appearance: now.settings.appearance.appearance(),
            system: now.system,
            typeface: Some(now.settings.appearance.typeface()),
            material: Material::Window,
            extent: RootExtent::Window,
            style { {SETTINGS_CSS} }
            SettingsView { page }
        }
    }
}

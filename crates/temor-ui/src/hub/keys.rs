//! Which workspace request a pane action asks for (PLAN §6, default keys). Pure.

use std::sync::Arc;

use chordkit::{ChangeCallback, Keymap, Overrides};
use dioxus::prelude::spawn;
use ds_core::vocab::Shortcut;
use temor_core::split::{Direction, Goto};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn};
use temor_pane::keys::{Action, KeyIssue, Owner, build_keymap};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::edge::{LOOK_EVERY, system_keys, watch_file};
use crate::input::terminal_shortcut_of;
use crate::settings::read_overrides;

/// The workspace request for a tab or split action, or `None` for an action that is not the
/// workspace's (`Action::owner`): a pane runs its own, and the window takes the rest.
pub fn workspace_input(action: Action) -> Option<WorkspaceIn> {
    if action.owner() != Owner::Workspace {
        return None;
    }
    Some(match action {
        Action::NewTab => WorkspaceIn::NewTab { cwd: None },
        Action::ClosePane => WorkspaceIn::ClosePane(None),
        Action::SplitRight => WorkspaceIn::Split {
            towards: Direction::Right,
        },
        Action::SplitDown => WorkspaceIn::Split {
            towards: Direction::Down,
        },
        Action::FocusLeft => WorkspaceIn::Goto(Goto::Spatial(Direction::Left)),
        Action::FocusRight => WorkspaceIn::Goto(Goto::Spatial(Direction::Right)),
        Action::FocusUp => WorkspaceIn::Goto(Goto::Spatial(Direction::Up)),
        Action::FocusDown => WorkspaceIn::Goto(Goto::Spatial(Direction::Down)),
        Action::ToggleZoom => WorkspaceIn::ToggleZoom,
        Action::Equalize => WorkspaceIn::Equalize,
        Action::ReopenTab => WorkspaceIn::Reopen,
        Action::SelectTab(n) => WorkspaceIn::Tabs(TabsIn::Select(TabSel::Nth(NthTab::new(n)?))),
        // A workspace action this table forgot would be dropped silently; a test walks every
        // bound action to catch it.
        _ => return None,
    })
}

impl HubRef {
    /// What a person presses to run `action` in a terminal window, as quire draws a chord.
    pub fn shortcut_of(&self, action: Action) -> Option<Shortcut> {
        terminal_shortcut_of(&self.0.borrow().launch.keymap, action)
    }

    /// `label` with the action's chord after it, as a Mac tooltip writes it (`New tab (⌘N)`), or
    /// `label` alone when the action has no chord.
    pub fn tooltip(&self, label: &str, action: Action) -> String {
        match temor_pane::keys::terminal_shortcut(&self.0.borrow().launch.keymap, action) {
            Some(chord) => format!("{label} ({chord})"),
            None => label.to_owned(),
        }
    }
}

impl Hub {
    /// Routes keys by `keymap` from now on, in the window and in every pane that is open, and
    /// remembers the person's changes it was built from.
    fn rekey(&mut self, overrides: Overrides, keymap: Keymap) {
        for slot in self.slots.values() {
            if let Some(session) = slot.borrow_mut().session.as_mut() {
                session.keymap = keymap.clone();
            }
        }
        self.launch.settings.keys = overrides;
        self.launch.keymap = keymap;
    }
}

/// What a reload of the keymap found: the person's changes it read, the keymap, and the problems.
type Reloaded = (Overrides, Keymap, Vec<KeyIssue>);

/// Loads the keymap again whenever the desktop's own shortcut settings or the person's own file
/// (`keys.conf`) change, for as long as the window lives (PLAN §6). Reading the desktop's settings
/// may run `dconf`, so the source and its watchers are set up on a thread of their own and the
/// reloads run on the watchers' threads; the window only swaps the result in. A platform with no
/// settings of its own to read still follows the file.
pub fn spawn_keys_loop(desk: Desk) {
    let (platform, ambient) = {
        let hub = desk.hub.0.borrow();
        (hub.launch.keymap.platform(), hub.launch.ambient.clone())
    };
    let (reloaded, mut inbox) = tokio::sync::mpsc::unbounded_channel::<Reloaded>();
    let (watching, watches) = tokio::sync::oneshot::channel();
    // A thread that cannot start leaves the keys as they were read at start.
    let _ = std::thread::Builder::new()
        .name("temor-keys".to_owned())
        .spawn(move || {
            let (keys, keys_path) = (system_keys(platform, &ambient), ambient.keys_path());
            let reload: Arc<dyn Fn() + Send + Sync> = {
                let (loader, path) = (keys.clone(), keys_path.clone());
                Arc::new(move || {
                    // A file that cannot be read now leaves the keys as they were.
                    let Ok((overrides, mut issues)) = read_overrides(&path) else {
                        return;
                    };
                    let (keymap, built) = build_keymap(loader.source(), platform, &overrides);
                    issues.extend(built);
                    let _ = reloaded.send((overrides, keymap, issues));
                })
            };
            let callback = |reload: &Arc<dyn Fn() + Send + Sync>| -> ChangeCallback {
                let reload = Arc::clone(reload);
                Box::new(move || reload())
            };
            // A source that cannot watch leaves the desktop's keys as they were read at start.
            let desktop_watch = keys.watch(callback(&reload)).ok();
            let file_watch = watch_file(&keys_path, LOOK_EVERY, callback(&reload)).ok();
            let _ = watching.send((desktop_watch, file_watch));
        });
    spawn(async move {
        // Kept for as long as the window listens; dropping them stops the watchers.
        let _watches = watches.await;
        while let Some((overrides, keymap, issues)) = inbox.recv().await {
            desk.hub.0.borrow_mut().rekey(overrides, keymap);
            for (line, issue) in issues {
                match line {
                    0 => desk.say(format!("Shortcuts: {issue}")),
                    line => desk.say(format!("Shortcuts, line {line}: {issue}")),
                }
            }
            desk.bump();
        }
    });
}

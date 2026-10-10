//! Which workspace request a pane action asks for (PLAN §6, default keys). Pure.

use chordkit::Keymap;
use dioxus::prelude::spawn;
use ds_core::vocab::Shortcut;
use temor_core::split::{Direction, Goto};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::edge::SystemKeys;
use crate::input::terminal_shortcut_of;

/// The workspace request for a tab or split action, or `None` for an action a pane runs itself.
pub fn workspace_input(action: Action) -> Option<WorkspaceIn> {
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
        // The command bar and the quick terminal are the window's own, not the workspace's
        // (`hub::bar`, `hub::quick`).
        Action::OpenBar
        | Action::ToggleQuick
        | Action::Copy
        | Action::Paste
        | Action::Find
        | Action::FindNext
        | Action::FindPrevious
        | Action::CloseFind
        | Action::PreviousPrompt
        | Action::NextPrompt
        // The window's own: the radar jumps (`Hub::jump_to_waiting`), and the Space switch goes
        // through the kit (`Desk::select_space`).
        | Action::JumpToWaiting
        | Action::SelectSpace(_) => return None,
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
    /// Routes keys by `keymap` from now on, in the window and in every pane that is open.
    fn rekey(&mut self, keymap: Keymap) {
        for slot in self.slots.values() {
            if let Some(session) = slot.borrow_mut().session.as_mut() {
                session.keymap = keymap.clone();
            }
        }
        self.launch.keymap = keymap;
    }
}

/// Loads the keymap again whenever the desktop's own shortcut settings change, for as long as
/// the window lives (PLAN §6). A platform with no settings to read starts nothing. The reload
/// runs on the watcher's thread (it may run `dconf`); the window only swaps the result in.
pub fn spawn_keys_loop(desk: Desk) {
    let (platform, overrides, keys) = {
        let hub = desk.hub.0.borrow();
        let platform = hub.launch.keymap.platform();
        let ambient = &hub.launch.ambient;
        let keys = SystemKeys::choose(platform, &ambient.config_home(), &ambient.runtime_dir);
        (platform, hub.launch.settings.keys.clone(), keys)
    };
    if keys == SystemKeys::Conventions {
        return;
    }
    let source = keys.source();
    let (reloaded, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    let loader = std::sync::Arc::clone(&source);
    // A source that cannot watch leaves the keys as they were read at start.
    let Ok(watch) = source.watch(Box::new(move || {
        let _ = reloaded.send(temor_pane::keys::build_keymap(
            &*loader, platform, &overrides,
        ));
    })) else {
        return;
    };
    spawn(async move {
        // Kept for as long as the window listens; dropping it stops the watcher.
        let _watch = watch;
        while let Some((keymap, issues)) = inbox.recv().await {
            desk.hub.0.borrow_mut().rekey(keymap);
            // Lines of the person's own file were reported at start; these are the desktop's.
            for (_, issue) in issues.into_iter().filter(|(line, _)| *line == 0) {
                desk.say(format!("Shortcuts: {issue}"));
            }
            desk.bump();
        }
    });
}

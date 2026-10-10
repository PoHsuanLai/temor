//! Which workspace request a pane action asks for (PLAN §6, default keys). Pure.

use ds_core::vocab::Shortcut;
use temor_core::split::{Direction, Goto};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use super::model::HubRef;
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

//! Which workspace request a pane action asks for (PLAN §6, default keys). Pure.

use temor_core::split::{Direction, Goto};
use temor_core::workspace::{NthTab, TabSel, TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

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
        Action::Copy
        | Action::Paste
        | Action::Find
        | Action::FindNext
        | Action::FindPrevious
        | Action::CloseFind
        | Action::PreviousPrompt
        | Action::NextPrompt
        // The Space switch is the window's (`Desk::select_space`), through the kit.
        | Action::SelectSpace(_) => return None,
    })
}

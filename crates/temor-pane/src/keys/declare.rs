//! temor's own actions and their portable defaults (PLAN §6), written with `Primary` so each
//! platform and terminal context gives them the right modifiers. Standard actions (copy, paste,
//! find, new, close) are not declared: the platform's conventions already bind them.

use super::action::Action;

/// The default chord text of every action temor declares, in registration order. Tabs 1 to 9
/// are `Primary+1` to `Primary+9`. The Space switch and "close find" have no entry: the desktop
/// owns Ctrl+digit, and Escape is the find bar's own key.
pub fn declared() -> Vec<(Action, String)> {
    let fixed: &[(Action, &str)] = &[
        (Action::SplitRight, "Primary+D"),
        (Action::SplitDown, "Primary+Shift+D"),
        (Action::FocusLeft, "Primary+Alt+Left"),
        (Action::FocusRight, "Primary+Alt+Right"),
        (Action::FocusUp, "Primary+Alt+Up"),
        (Action::FocusDown, "Primary+Alt+Down"),
        (Action::PreviousPrompt, "Primary+Up"),
        (Action::NextPrompt, "Primary+Down"),
        (Action::ToggleZoom, "Primary+Shift+Enter"),
        (Action::Equalize, "Primary+Ctrl+="),
        (Action::ReopenTab, "Primary+Shift+T"),
    ];
    let tabs = (1..=9u8).map(|n| (Action::SelectTab(n), format!("Primary+{n}")));
    fixed
        .iter()
        .map(|(action, text)| (*action, (*text).to_owned()))
        .chain(tabs)
        .collect()
}

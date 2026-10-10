//! temor's own actions and their portable defaults (PLAN §6), written with `Primary` so each
//! platform and terminal context gives them the right modifiers. Standard actions temor runs
//! (copy, paste, find, close) are not declared: the platform's conventions already bind them.

use chordkit::StandardAction;

use super::action::Action;

/// One action temor registers: its default, a fallback for contexts where the default has no
/// live form, and the offered-only standard actions it takes the chord of (temor has no font
/// panel, so ⌘T is a new tab, as in other terminals; nor a separate "new", so ⌘N is free).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub action: Action,
    pub chord: String,
    pub fallback: Option<&'static str>,
    pub forgo: &'static [StandardAction],
}

/// Every action temor declares, in registration order. Tabs 1 to 9 are `Primary+1` to
/// `Primary+9`. The Space switch and "close find" have no entry: the desktop owns Ctrl+digit,
/// and Escape is the find bar's own key.
pub fn declared() -> Vec<Declared> {
    const NEW_TAB_FORGOES: &[StandardAction] = &[StandardAction::ShowFonts, StandardAction::New];
    let fixed: &[(
        Action,
        &str,
        Option<&'static str>,
        &'static [StandardAction],
    )] = &[
        (Action::NewTab, "Primary+T", None, NEW_TAB_FORGOES),
        (Action::SplitRight, "Primary+D", None, &[]),
        (Action::SplitDown, "Primary+Shift+D", None, &[]),
        (Action::FocusLeft, "Primary+Alt+Left", None, &[]),
        (Action::FocusRight, "Primary+Alt+Right", None, &[]),
        (Action::FocusUp, "Primary+Alt+Up", None, &[]),
        (Action::FocusDown, "Primary+Alt+Down", None, &[]),
        (Action::PreviousPrompt, "Primary+Up", None, &[]),
        (Action::NextPrompt, "Primary+Down", None, &[]),
        (Action::ToggleZoom, "Primary+Shift+Enter", None, &[]),
        // Where Primary is Ctrl+Shift, `=` with Shift is `+` under plain Ctrl, the program's:
        // there equalize is Primary+Alt+0 (Ctrl+Shift+Alt+0).
        (
            Action::Equalize,
            "Primary+Ctrl+=",
            Some("Primary+Alt+0"),
            &[],
        ),
        (Action::ReopenTab, "Primary+Shift+T", None, &[]),
        (Action::JumpToWaiting, "Primary+Shift+U", None, &[]),
    ];
    let tabs = (1..=9u8).map(|n| Declared {
        action: Action::SelectTab(n),
        chord: format!("Primary+{n}"),
        fallback: None,
        forgo: &[],
    });
    fixed
        .iter()
        .map(|(action, chord, fallback, forgo)| Declared {
            action: *action,
            chord: (*chord).to_owned(),
            fallback: *fallback,
            forgo,
        })
        .chain(tabs)
        .collect()
}

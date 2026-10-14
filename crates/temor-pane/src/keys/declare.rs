//! temor's own actions and their portable defaults (PLAN §6), written with `Primary` so each
//! platform and terminal context gives them the right modifiers. Standard actions temor runs
//! (copy, paste, find, close) are not declared: the platform's conventions already bind them.

use chordkit::StandardAction;

use super::action::Action;

/// One action temor registers: its default, a fallback for contexts where the default has no
/// live form, and the offered-only standard actions it takes the chord of (temor has no font
/// panel, so ⌘T is the command bar, as in a browser; ⌘N is the standard "new", a plain tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub action: Action,
    pub chord: String,
    pub fallback: Option<&'static str>,
    pub forgo: &'static [StandardAction],
}

/// Every action temor declares, in registration order. Tabs 1 to 9 are `Primary+1` to
/// `Primary+9`. The Space switch and "close find" have no entry: the desktop owns Ctrl+digit,
/// and Escape is the find bar's own key. Nor do the bookmark steps: Alt+Up and Alt+Down are
/// theirs only while the pane has a bookmark and sits at its prompt (`Session::pick_key`).
pub fn declared() -> Vec<Declared> {
    const OPEN_BAR_FORGOES: &[StandardAction] = &[StandardAction::ShowFonts];
    const NEW_TAB_FORGOES: &[StandardAction] = &[StandardAction::New];
    // A terminal has no colour panel and no text to embolden or italicise: Warp's block keys
    // take those chords.
    const COPY_COMMANDS_FORGOES: &[StandardAction] = &[StandardAction::ShowColors];
    const BOLD_FORGOES: &[StandardAction] = &[StandardAction::Bold];
    const ITALIC_FORGOES: &[StandardAction] = &[StandardAction::Italic];
    let fixed: &[(
        Action,
        &str,
        Option<&'static str>,
        &'static [StandardAction],
    )] = &[
        (Action::OpenBar, "Primary+T", None, OPEN_BAR_FORGOES),
        (Action::NewTab, "Primary+N", None, NEW_TAB_FORGOES),
        // Primary+Shift+Y: no convention of chordkit's binds it (Primary+` is "next window").
        (Action::ToggleQuick, "Primary+Shift+Y", None, &[]),
        (Action::SplitRight, "Primary+D", None, &[]),
        (Action::SplitDown, "Primary+Shift+D", None, &[]),
        (Action::FocusLeft, "Primary+Alt+Left", None, &[]),
        (Action::FocusRight, "Primary+Alt+Right", None, &[]),
        (Action::FocusUp, "Primary+Alt+Up", None, &[]),
        (Action::FocusDown, "Primary+Alt+Down", None, &[]),
        (Action::PreviousPrompt, "Primary+Up", None, &[]),
        (Action::NextPrompt, "Primary+Down", None, &[]),
        // Warp's block keys: ⌘⇧C copies the picked commands, ⌘B bookmarks, ⌘I puts the commands
        // back in the prompt (⌘A, picking them all, is the standard select all). Outputs alone
        // are ⌘⇧O: Warp's ⌥⇧⌘C has the same Ctrl+Shift+Alt form as ⌘⇧C where Ctrl+Shift is the
        // Primary.
        (
            Action::CopyCommands,
            "Primary+Shift+C",
            None,
            COPY_COMMANDS_FORGOES,
        ),
        (Action::CopyOutputs, "Primary+Shift+O", None, &[]),
        (Action::ToggleBookmark, "Primary+B", None, BOLD_FORGOES),
        (Action::ReinputCommands, "Primary+I", None, ITALIC_FORGOES),
        (Action::ToggleZoom, "Primary+Shift+Enter", None, &[]),
        // Ctrl+= is the text size (Bigger). Where Primary is Ctrl+Shift, `=` with Shift and Alt is
        // `+` under Ctrl and Alt, the program's: there equalize is Primary+Alt+0
        // (Ctrl+Shift+Alt+0).
        (
            Action::Equalize,
            "Primary+Alt+=",
            Some("Primary+Alt+0"),
            &[],
        ),
        (Action::ReopenTab, "Primary+Shift+T", None, &[]),
        (Action::JumpToWaiting, "Primary+Shift+U", None, &[]),
        // Safari's chord for its sidebar; no convention of chordkit's binds it.
        (Action::ToggleSidebar, "Primary+Shift+L", None, &[]),
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

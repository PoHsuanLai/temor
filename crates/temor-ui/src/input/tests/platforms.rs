//! Where ⌘ chords go on each platform.

use chordkit::Platform;
use dioxus::html::{Code, Key, Modifiers as Held};
use temor_pane::keys::Action;

use super::super::route::FindBar;
use super::table::{KDE, OURS, Went, ch, check, row};

/// Key events as our desktop delivers them, through the stroke and the router: ⌘ arrives as
/// Super, or as Ctrl+Shift when keycap remaps it for a terminal window. Shifted keys come with
/// the shifted character as the logical key, and still match their base key.
#[test]
fn our_desktop_sends_command_as_super_or_ctrl_shift() {
    let cs = Held::CONTROL | Held::SHIFT;
    let rows = [
        row(
            "⌘C as Ctrl+Shift+C",
            ch("C"),
            Code::KeyC,
            cs,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘C as Super+C",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘C as Meta+C",
            ch("c"),
            Code::KeyC,
            Held::META,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘1 under Shift",
            ch("!"),
            Code::Digit1,
            cs,
            Went::Action(Action::SelectTab(1)),
        ),
        row(
            "⌘9 under Shift",
            ch("("),
            Code::Digit9,
            cs,
            Went::Action(Action::SelectTab(9)),
        ),
        row(
            "⌘W as Ctrl+Shift+W",
            ch("W"),
            Code::KeyW,
            cs,
            Went::Action(Action::ClosePane),
        ),
        row(
            "⌘T as Ctrl+Shift+T",
            ch("T"),
            Code::KeyT,
            cs,
            Went::Action(Action::OpenBar),
        ),
        row(
            "⌘D as Ctrl+Shift+D",
            ch("D"),
            Code::KeyD,
            cs,
            Went::Action(Action::SplitRight),
        ),
        row(
            "⌘⇧D as Super+Shift+D",
            ch("D"),
            Code::KeyD,
            Held::SUPER | Held::SHIFT,
            Went::Action(Action::SplitDown),
        ),
        row(
            "⌥⌘= as Alt+Super+=",
            ch("="),
            Code::Equal,
            Held::ALT | Held::SUPER,
            Went::Action(Action::Equalize),
        ),
        row(
            "⌘= as Super+= zooms in",
            ch("="),
            Code::Equal,
            Held::SUPER,
            Went::Action(Action::ZoomIn),
        ),
        // A ⌘ chord with no action is dropped, never a control byte for the program.
        row("⌘K has no action", ch("K"), Code::KeyK, cs, Went::Dropped),
        row(
            "Super+K has no action",
            ch("k"),
            Code::KeyK,
            Held::SUPER,
            Went::Dropped,
        ),
        row(
            "⌘N as Ctrl+Shift+N is a plain tab",
            ch("N"),
            Code::KeyN,
            cs,
            Went::Action(Action::NewTab),
        ),
        // The program's own keys.
        row(
            "Ctrl+C is the interrupt",
            ch("c"),
            Code::KeyC,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+D is end of input",
            ch("d"),
            Code::KeyD,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+1 is the desktop's",
            ch("1"),
            Code::Digit1,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+Space is the input method's",
            ch(" "),
            Code::Space,
            Held::CONTROL,
            Went::Program,
        ),
        // The desktop handles these before a window sees them.
        row(
            "Super alone",
            Key::Meta,
            Code::MetaLeft,
            Held::SUPER,
            Went::Program,
        ),
        row(
            "Super+Space",
            ch(" "),
            Code::Space,
            Held::SUPER,
            Went::Program,
        ),
        row("Super+Tab", Key::Tab, Code::Tab, Held::SUPER, Went::Program),
    ];
    check(OURS, FindBar::Closed, &rows);
}

#[test]
fn windows_and_kde_use_ctrl_shift_and_leave_plain_ctrl_and_super_alone() {
    let cs = Held::CONTROL | Held::SHIFT;
    let rows = [
        row(
            "Ctrl+Shift+C",
            ch("C"),
            Code::KeyC,
            cs,
            Went::Action(Action::Copy),
        ),
        row(
            "Ctrl+Shift+D",
            ch("D"),
            Code::KeyD,
            cs,
            Went::Action(Action::SplitRight),
        ),
        row(
            "Ctrl+Shift+Alt+D",
            ch("D"),
            Code::KeyD,
            cs | Held::ALT,
            Went::Action(Action::SplitDown),
        ),
        row(
            "Ctrl+Shift+1",
            ch("!"),
            Code::Digit1,
            cs,
            Went::Action(Action::SelectTab(1)),
        ),
        row(
            "Ctrl+Shift+K has no action",
            ch("K"),
            Code::KeyK,
            cs,
            Went::Dropped,
        ),
        row(
            "Ctrl+C is the interrupt",
            ch("c"),
            Code::KeyC,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+D is end of input",
            ch("d"),
            Code::KeyD,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+1",
            ch("1"),
            Code::Digit1,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Super+C is not a copy here",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Program,
        ),
        row("Super+K", ch("k"), Code::KeyK, Held::SUPER, Went::Program),
    ];
    for platform in [Platform::Windows, KDE] {
        check(platform, FindBar::Closed, &rows);
    }
}

#[test]
fn macos_uses_command_only() {
    let rows = [
        row(
            "⌘C",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘K has no action",
            ch("k"),
            Code::KeyK,
            Held::SUPER,
            Went::Dropped,
        ),
        row(
            "Ctrl+Shift+C is the program's",
            ch("C"),
            Code::KeyC,
            Held::CONTROL | Held::SHIFT,
            Went::Program,
        ),
        row("Ctrl+C", ch("c"), Code::KeyC, Held::CONTROL, Went::Program),
    ];
    check(Platform::MacOs, FindBar::Closed, &rows);
}

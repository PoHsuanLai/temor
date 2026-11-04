use super::*;

fn under(file: &str) -> Under {
    Under {
        file: PathBuf::from(file),
        x: 10.0,
        y: 20.0,
    }
}

fn looked(command: Command, file: Option<&str>) -> PeekEvent {
    PeekEvent::Looked {
        command,
        under: file.map(under),
    }
}

fn showing(file: &str) -> Peek {
    Peek::Showing(PathBuf::from(file))
}

fn spent(file: Option<&str>) -> Peek {
    Peek::Spent(file.map(PathBuf::from))
}

fn typed(over: Option<&str>) -> PeekEvent {
    PeekEvent::Typed {
        over: over.map(PathBuf::from),
    }
}

#[test]
fn show_and_hide_decisions() {
    use Command::{Held, Up};
    // (state, event, next state, effect)
    let table = [
        // ⌘ down over a path shows; ⌘ down over text, or up over a path, does nothing.
        (
            Peek::Idle,
            looked(Held, Some("/a")),
            showing("/a"),
            PeekEffect::Show(under("/a")),
        ),
        (
            Peek::Idle,
            looked(Held, None),
            Peek::Idle,
            PeekEffect::Nothing,
        ),
        (
            Peek::Idle,
            looked(Up, None),
            Peek::Idle,
            PeekEffect::Nothing,
        ),
        // Resting on the same path keeps the card; another path replaces it.
        (
            showing("/a"),
            looked(Held, Some("/a")),
            showing("/a"),
            PeekEffect::Nothing,
        ),
        (
            showing("/a"),
            looked(Held, Some("/b")),
            showing("/b"),
            PeekEffect::Show(under("/b")),
        ),
        // ⌘ up or moving off the path hides.
        (
            showing("/a"),
            looked(Up, None),
            Peek::Idle,
            PeekEffect::Hide,
        ),
        (
            showing("/a"),
            looked(Held, None),
            Peek::Idle,
            PeekEffect::Hide,
        ),
        // The pointer leaving or pressing hides.
        (showing("/a"), PeekEvent::Gone, Peek::Idle, PeekEffect::Hide),
        (Peek::Idle, PeekEvent::Gone, Peek::Idle, PeekEffect::Nothing),
        // A typed key hides and keeps the card away while ⌘ stays down on that path.
        (
            showing("/a"),
            typed(Some("/a")),
            spent(Some("/a")),
            PeekEffect::Hide,
        ),
        (
            spent(Some("/a")),
            looked(Held, Some("/a")),
            spent(Some("/a")),
            PeekEffect::Nothing,
        ),
        (
            Peek::Idle,
            typed(Some("/a")),
            spent(Some("/a")),
            PeekEffect::Nothing,
        ),
        (
            Peek::Idle,
            looked(Held, Some("/a")),
            showing("/a"),
            PeekEffect::Show(under("/a")),
        ),
        (Peek::Idle, typed(None), Peek::Idle, PeekEffect::Nothing),
        // It comes back once ⌘ is let go, the pointer leaves the path or moves to another.
        (
            spent(Some("/a")),
            looked(Up, Some("/a")),
            Peek::Idle,
            PeekEffect::Nothing,
        ),
        (
            spent(Some("/a")),
            looked(Held, None),
            Peek::Idle,
            PeekEffect::Nothing,
        ),
        (
            spent(Some("/a")),
            looked(Held, Some("/b")),
            showing("/b"),
            PeekEffect::Show(under("/b")),
        ),
        (
            spent(Some("/a")),
            PeekEvent::Gone,
            Peek::Idle,
            PeekEffect::Nothing,
        ),
    ];
    for (state, event, next, effect) in table {
        let got = step(&state, event.clone());
        assert_eq!(got, (next, effect), "{state:?} on {event:?}");
    }
}

#[test]
fn command_is_the_click_chord() {
    use temor_panes::vt::Modifiers;
    let both = Modifiers::CTRL.with(Modifiers::SHIFT);
    let table = [
        (Modifiers::NONE, Command::Up),
        (Modifiers::CTRL, Command::Up),
        (Modifiers::SHIFT, Command::Up),
        (both, Command::Held),
        (both.with(Modifiers::ALT), Command::Held),
    ];
    for (mods, want) in table {
        assert_eq!(Command::of(mods), want, "{mods:?}");
    }
}

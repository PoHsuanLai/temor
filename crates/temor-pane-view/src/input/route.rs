//! Where a key goes (PLAN §6): a key the keymap binds in a terminal runs its action; a key in
//! the Primary form with no action is dropped, as Ghostty does; every other key is encoded for
//! the program, unless the input method is composing, which owns the keys until it is done. When
//! the program has turned on the kitty keyboard flags it asked for those chords, so they reach it.
//! Pure: no window, no terminal.

use chordkit::{Context, Key as ChordKey, Keymap, NamedKey};
use temor_panes::pane::keys::Action;
use temor_panes::vt::{KeyAction, KeyInput, KittyKeyFlags};

/// A key as the window heard it: the press as chordkit reads it (when it has a name there) and
/// what the program would get.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stroke {
    pub input: Option<chordkit::KeyInput>,
    pub key: KeyInput,
}

/// Whether the find bar is open: its keys (Escape, next, previous) belong to it only then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindBar {
    Open,
    Closed,
}

/// Whether the program has turned on the kitty keyboard flags (any of them): then a chord with no
/// action is the program's to read, not a control byte to drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KittyKeys {
    On,
    Off,
}

impl KittyKeys {
    /// `On` when the program pushed any flag.
    pub fn of(flags: KittyKeyFlags) -> Self {
        match flags == KittyKeyFlags::DISABLED {
            true => Self::Off,
            false => Self::On,
        }
    }
}

/// Where a key goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// The pane runs this action; the program never sees the key.
    Action(Action),
    /// The program gets the key, encoded by the terminal's modes.
    Shell(KeyInput),
    /// Nothing: the input method is composing, or the key means nothing here.
    Ignored,
}

/// Routes one key event (PLAN §4: a machine never takes a key the shell would get).
pub fn route(stroke: Stroke, keymap: &Keymap, find: FindBar) -> Routed {
    route_with(stroke, keymap, find, KittyKeys::Off)
}

/// [`route`], knowing whether the program turned on the kitty keyboard flags.
pub fn route_with(stroke: Stroke, keymap: &Keymap, find: FindBar, kitty: KittyKeys) -> Routed {
    if stroke.key.composing {
        return Routed::Ignored;
    }
    if stroke.key.action == KeyAction::Release {
        return Routed::Shell(stroke.key);
    }
    let Some(input) = stroke.input else {
        return Routed::Shell(stroke.key);
    };
    // Escape closes the find bar while it is open; that is the bar's own key, not a keymap entry.
    if find == FindBar::Open && is_plain_escape(&input) {
        return Routed::Action(Action::CloseFind);
    }
    let bound = keymap
        .resolve(&input, Context::Terminal)
        .and_then(|named| Action::from_chordkit(&named));
    match (bound, find) {
        (Some(Action::CloseFind), FindBar::Open) => Routed::Action(Action::CloseFind),
        // The find keys are the bar's only while it is open; elsewhere they are any other key.
        (Some(Action::CloseFind | Action::FindNext | Action::FindPrevious), FindBar::Closed)
        | (None, _) => unbound(stroke.key, &input, keymap, kitty),
        (Some(action), _) => Routed::Action(action),
    }
}

/// A key with no action: dropped when it is in the Primary form (it would reach the program as a
/// control byte; Ghostty drops these too), else the program's. Under the kitty keyboard flags the
/// program asked for these chords, so it gets them.
fn unbound(key: KeyInput, input: &chordkit::KeyInput, keymap: &Keymap, kitty: KittyKeys) -> Routed {
    match (keymap.is_primary_form(input, Context::Terminal), kitty) {
        (true, KittyKeys::Off) => Routed::Ignored,
        (true, KittyKeys::On) | (false, _) => Routed::Shell(key),
    }
}

fn is_plain_escape(input: &chordkit::KeyInput) -> bool {
    input.modifiers.is_empty() && input.key == ChordKey::Named(NamedKey::Escape)
}

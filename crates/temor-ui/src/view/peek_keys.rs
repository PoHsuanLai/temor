//! What a key event means for the peek card (PLAN §10): a modifier going down or up moves ⌘, any
//! other key pressed is typing. The platform tells a modifier key's own press without its bit
//! and its release with it, so the bit is applied here.

use dioxus::html::{Key, Modifiers as HeldMods};
use ds::edit::raw_key::{KeyPhase, RawKey};

use crate::hub::peek_step::Command;
use crate::input::vt_mods;

/// What a key event says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySays {
    /// ⌘ is now held or up; look at the pointer again.
    Look(Command),
    /// A key was typed.
    Typed,
}

/// The modifier bit a modifier key stands for.
fn bit_of(key: &Key) -> Option<HeldMods> {
    Some(match key {
        Key::Control => HeldMods::CONTROL,
        Key::Shift => HeldMods::SHIFT,
        Key::Alt => HeldMods::ALT,
        Key::AltGraph => HeldMods::ALT_GRAPH,
        Key::Meta => HeldMods::META,
        Key::Super => HeldMods::SUPER,
        Key::CapsLock => HeldMods::CAPS_LOCK,
        Key::NumLock => HeldMods::NUM_LOCK,
        Key::ScrollLock => HeldMods::SCROLL_LOCK,
        Key::Fn => HeldMods::FN,
        Key::FnLock => HeldMods::FN_LOCK,
        Key::Hyper => HeldMods::HYPER,
        Key::Symbol => HeldMods::SYMBOL,
        Key::SymbolLock => HeldMods::SYMBOL_LOCK,
        _ => return None,
    })
}

/// What `raw` means for the peek.
pub fn says(raw: &RawKey) -> KeySays {
    match (bit_of(&raw.key), raw.phase) {
        (Some(bit), KeyPhase::Release) => KeySays::Look(Command::of(vt_mods(raw.modifiers - bit))),
        (Some(bit), KeyPhase::Press | KeyPhase::Repeat) => {
            KeySays::Look(Command::of(vt_mods(raw.modifiers | bit)))
        }
        (None, KeyPhase::Press | KeyPhase::Repeat) => KeySays::Typed,
        (None, KeyPhase::Release) => KeySays::Look(Command::of(vt_mods(raw.modifiers))),
    }
}

#[cfg(test)]
mod tests {
    use dioxus::html::{Code, Location};

    use super::*;

    fn key(phase: KeyPhase, key: Key, held: HeldMods) -> RawKey {
        RawKey {
            phase,
            key,
            code: Code::Unidentified,
            modifiers: held,
            location: Location::Standard,
            text: None,
            unshifted: None,
        }
    }

    #[test]
    fn modifiers_move_command_and_other_keys_are_typing() {
        use Command::{Held, Up};
        use KeyPhase::{Press, Release, Repeat};
        let ctrl = HeldMods::CONTROL;
        let both = ctrl | HeldMods::SHIFT;
        let c = || Key::Character("c".to_owned());
        // (event, what it says). A modifier's own press comes without its bit, its release with it.
        let table = [
            (key(Press, Key::Shift, ctrl), KeySays::Look(Held)),
            (
                key(Press, Key::Control, HeldMods::empty()),
                KeySays::Look(Up),
            ),
            (key(Repeat, Key::Shift, ctrl), KeySays::Look(Held)),
            (key(Release, Key::Shift, both), KeySays::Look(Up)),
            (key(Release, Key::Control, both), KeySays::Look(Up)),
            (
                key(Release, Key::Alt, both | HeldMods::ALT),
                KeySays::Look(Held),
            ),
            (key(Press, c(), both), KeySays::Typed),
            (key(Repeat, c(), HeldMods::empty()), KeySays::Typed),
            (key(Release, c(), both), KeySays::Look(Held)),
            (key(Release, c(), ctrl), KeySays::Look(Up)),
        ];
        for (raw, want) in table {
            assert_eq!(says(&raw), want, "{raw:?}");
        }
    }
}

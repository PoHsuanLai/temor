//! A chord the person pressed, written for `keys.conf`: with `Primary` where the platform's
//! primary modifiers were held, so the line means the same on every machine the file reaches.

use chordkit::{Chord, Context, DefaultChord, Platform, PrimaryUse};

/// `chord`, as pressed in a terminal window of `platform`, in the form `keys.conf` holds
/// (`Primary+Alt+N`). It uses `Primary` only when that resolves back to exactly `chord`;
/// otherwise the chord is written as pressed.
pub fn portable_chord(platform: Platform, chord: Chord) -> String {
    let primary = platform.primary(Context::Terminal);
    if !primary.is_empty() && chord.modifiers().contains_all(primary) {
        let base = Chord::new(chord.modifiers().minus(primary), chord.key());
        let held = DefaultChord::new(PrimaryUse::Held, base);
        if held.resolve(platform, Context::Terminal) == Some(chord) {
            return held.canonical();
        }
    }
    DefaultChord::from(chord).canonical()
}

#[cfg(test)]
mod tests {
    use chordkit::{Chord, Key, Modifier, Modifiers, Platform};

    use super::portable_chord;

    fn chord(mods: &[Modifier], key: char) -> Chord {
        Chord::new(mods.iter().copied().collect::<Modifiers>(), Key::Char(key))
    }

    #[test]
    fn the_primary_modifiers_are_written_as_primary() {
        let rows = [
            (
                Platform::Windows,
                chord(&[Modifier::Ctrl, Modifier::Shift, Modifier::Alt], 'n'),
                "Primary+Alt+N",
            ),
            (Platform::Windows, chord(&[Modifier::Alt], 'n'), "Alt+N"),
        ];
        for (platform, pressed, want) in rows {
            assert_eq!(portable_chord(platform, pressed), want, "{pressed:?}");
        }
    }
}

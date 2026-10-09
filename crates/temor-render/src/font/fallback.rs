//! What a grapheme needs from a font: emoji presentation, and the script to look a fallback up by.

use icu_properties::props::{EmojiPresentation, ExtendedPictographic, Script};
use icu_properties::{CodePointMapData, CodePointSetData, PropertyNamesShort};

/// Variation selector 16: "show as emoji".
const VS16: char = '\u{fe0f}';
/// Variation selector 15: "show as text".
const VS15: char = '\u{fe0e}';

/// How a cluster wants to be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Presentation {
    Text,
    Emoji,
}

/// Emoji when the cluster asks for it (VS16, a ZWJ sequence of pictographs, a skin tone) or its
/// first character defaults to emoji presentation; text otherwise and after VS15.
pub(crate) fn presentation(text: &str) -> Presentation {
    if text.contains(VS15) {
        return Presentation::Text;
    }
    let Some(first) = text.chars().next() else {
        return Presentation::Text;
    };
    let emoji_default = CodePointSetData::new::<EmojiPresentation>().contains(first);
    let pictographic = CodePointSetData::new::<ExtendedPictographic>().contains(first);
    let asks = text.contains(VS16) || (pictographic && text.chars().count() > 1);
    if emoji_default || asks {
        Presentation::Emoji
    } else {
        Presentation::Text
    }
}

/// The ISO 15924 script of `c` (`Latn`, `Hani`, ...), for fontique's fallback lookup; `None`
/// for characters common to every script.
pub(crate) fn script_of(c: char) -> Option<fontique::Script> {
    let script = CodePointMapData::<Script>::new().get(c);
    if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
        return None;
    }
    let name = PropertyNamesShort::<Script>::new().get(script)?;
    let bytes: [u8; 4] = name.as_bytes().try_into().ok()?;
    Some(fontique::Script::from_bytes(bytes))
}

/// The characters a face must map for the cluster to be drawn from it: everything except
/// joiners, variation selectors and skin-tone modifiers (a face may compose those).
pub(crate) fn needs(c: char) -> bool {
    !matches!(c, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}' | '\u{1f3fb}'..='\u{1f3ff}')
}

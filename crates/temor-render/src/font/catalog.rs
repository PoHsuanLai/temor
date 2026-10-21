//! The fonts this computer has, for a picker: every family, which of them are monospace, and which
//! family "the system's" resolves to.
//!
//! fontique has no monospace flag, so a family is monospace when its regular face gives `i`, `M`
//! and `W` the same advance. Reading every family's file takes a moment; the catalog is built once
//! per process ([`font_catalog`]) and [`warm_font_catalog`] builds it on a thread of its own at start.

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use fontique::{Collection, CollectionOptions, FamilyInfo, SourceCache};
use swash::FontRef;

use super::set::default_monospace_family;

/// What the system's fonts are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontCatalog {
    all: BTreeSet<String>,
    monospace: BTreeSet<String>,
    default: Option<String>,
}

impl FontCatalog {
    /// Reads the system's fonts.
    fn read() -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: true,
        });
        let mut cache = SourceCache::default();
        let names: Vec<String> = collection
            .family_names()
            .filter(|name| !name.starts_with('.'))
            .map(str::to_owned)
            .collect();
        let mut all = BTreeSet::new();
        let mut monospace = BTreeSet::new();
        for name in names {
            let Some(info) = collection.family_by_name(&name) else {
                continue;
            };
            if is_monospace(&info, &mut cache) {
                monospace.insert(name.clone());
            }
            all.insert(name);
        }
        let default = default_monospace_family(&mut collection).and_then(|id| {
            let name = collection.family_name(id)?.to_owned();
            all.contains(&name).then_some(name)
        });
        Self {
            all,
            monospace,
            default,
        }
    }

    /// Every family, by name, in order.
    pub fn families(&self) -> impl Iterator<Item = &str> {
        self.all.iter().map(String::as_str)
    }

    /// The monospace families, by name, in order.
    pub fn monospace(&self) -> impl Iterator<Item = &str> {
        self.monospace.iter().map(String::as_str)
    }

    /// Whether a family called `name` is installed.
    pub fn has(&self, name: &str) -> bool {
        self.all.contains(name)
    }

    /// Whether the family `name` is monospace.
    pub fn is_monospace(&self, name: &str) -> bool {
        self.monospace.contains(name)
    }

    /// The family the terminal draws with when the person asks for the system's.
    pub fn default_family(&self) -> Option<&str> {
        self.default.as_deref()
    }
}

/// Whether the family's regular face is fixed pitch: `i`, `M` and `W` all advance the same.
fn is_monospace(info: &FamilyInfo, cache: &mut SourceCache) -> bool {
    let Some(font) = info.default_font() else {
        return false;
    };
    let Some(blob) = font.load(Some(cache)) else {
        return false;
    };
    let Some(face) = FontRef::from_index(blob.as_ref(), font.index() as usize) else {
        return false;
    };
    fixed_pitch(face)
}

/// Whether `i`, `M` and `W` all advance the same in `face`.
fn fixed_pitch(face: FontRef<'_>) -> bool {
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(&[]);
    let advance = |c: char| {
        let glyph = charmap.map(c);
        (glyph != 0).then(|| metrics.advance_width(glyph))
    };
    match (advance('i'), advance('M'), advance('W')) {
        (Some(i), Some(m), Some(w)) => m > 0.0 && (i - m).abs() < 0.5 && (w - m).abs() < 0.5,
        _ => false,
    }
}

static CATALOG: OnceLock<Arc<FontCatalog>> = OnceLock::new();

/// The system's fonts, read on the first call and kept. A call made while [`warm_font_catalog`]
/// is still reading waits for it.
pub fn font_catalog() -> Arc<FontCatalog> {
    Arc::clone(CATALOG.get_or_init(|| Arc::new(FontCatalog::read())))
}

/// Starts reading the system's fonts on a thread of its own (once per process), so a picker opened later does not
/// wait for the disk.
pub fn warm_font_catalog() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) || CATALOG.get().is_some() {
        return;
    }
    std::thread::Builder::new()
        .name("temor-font-catalog".to_owned())
        .spawn(|| {
            font_catalog();
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_pitch_face_is_monospace_and_a_face_without_letters_is_not() {
        let mono = include_bytes!("../../tests/fonts/NotoSansMono-Regular-subset.ttf");
        let emoji = include_bytes!("../../tests/fonts/NotoColorEmoji-subset.ttf");
        assert!(fixed_pitch(FontRef::from_index(mono, 0).expect("font")));
        assert!(!fixed_pitch(FontRef::from_index(emoji, 0).expect("font")));
    }

    #[test]
    fn the_catalog_agrees_with_itself() {
        let catalog = font_catalog();
        for name in catalog.monospace() {
            assert!(catalog.has(name), "{name}");
            assert!(catalog.is_monospace(name), "{name}");
        }
        if let Some(default) = catalog.default_family() {
            assert!(catalog.has(default), "{default}");
        }
    }
}

//! `FontSet`: the chosen family's four cuts plus fallbacks, found through fontique.

use std::collections::HashMap;
use std::sync::Arc;

use fontique::{
    Blob, Collection, CollectionOptions, FallbackKey, FamilyId, FamilyInfo, FontStyle, FontWeight,
    FontWidth, GenericFamily, SourceCache,
};

use super::config::{FaceStyle, FontConfig, FontFamily, FontSource, Slant, Weight};
use super::face::{ColorFormat, Embolden, Face, FaceId, FaceRole, Skew};
use super::fallback::{Presentation, needs, presentation, script_of};
use crate::error::RenderError;
use crate::metrics::FontSize;

/// Where a `FontSet` looks for fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lookup {
    System,
    Bundled,
}

/// The fonts a renderer draws with: the family's regular, bold, italic and bold-italic cuts
/// (synthesised where the family lacks one) and, per character, the first fallback face that
/// has it. Fallback faces load lazily and stay cached.
pub struct FontSet {
    collection: Collection,
    cache: SourceCache,
    lookup: Lookup,
    size: FontSize,
    faces: Vec<Face>,
    primary: [FaceId; 4],
    /// Bundled families in the order they were given.
    bundled: Vec<FamilyId>,
    families: HashMap<FamilyId, Option<FaceId>>,
    chosen: HashMap<(String, Presentation, FaceStyle), FaceId>,
}

impl std::fmt::Debug for FontSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontSet")
            .field("faces", &self.faces.len())
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl FontSet {
    /// Loads `config.family` from `source`. Fails only when no font at all can be found.
    pub fn load(config: &FontConfig, source: FontSource) -> Result<Self, RenderError> {
        let lookup = match source {
            FontSource::System => Lookup::System,
            FontSource::Bundled(_) => Lookup::Bundled,
        };
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: lookup == Lookup::System,
        });
        let mut bundled = Vec::new();
        if let FontSource::Bundled(files) = source {
            for (index, bytes) in files.into_iter().enumerate() {
                if swash::FontRef::from_index(&bytes, 0).is_none() {
                    return Err(RenderError::BadFontData { index });
                }
                let blob = Blob::new(Arc::new(bytes));
                for (family, _) in collection.register_fonts(blob, None) {
                    if !bundled.contains(&family) {
                        bundled.push(family);
                    }
                }
            }
        }
        let not_found = || RenderError::FontNotFound {
            family: match &config.family {
                FontFamily::Monospace => "monospace".to_owned(),
                FontFamily::Named(name) => name.clone(),
            },
        };
        let family =
            primary_family(&mut collection, &config.family, &bundled).ok_or_else(not_found)?;
        let mut set = Self {
            collection,
            cache: SourceCache::default(),
            lookup,
            size: config.size,
            faces: Vec::new(),
            primary: [FaceId(0); 4],
            bundled,
            families: HashMap::new(),
            chosen: HashMap::new(),
        };
        for style in FaceStyle::ALL {
            let face = set.cut(&family, style).ok_or_else(not_found)?;
            set.primary[style.index()] = set.push(face);
        }
        Ok(set)
    }

    pub fn size(&self) -> FontSize {
        self.size
    }

    pub(crate) fn face(&self, id: FaceId) -> &Face {
        // Ids only come from `push`, so they index `faces`; the first primary cut is the floor.
        &self.faces[usize::from(id.0).min(self.faces.len() - 1)]
    }

    pub(crate) fn primary(&self, style: FaceStyle) -> FaceId {
        self.primary[style.index()]
    }

    /// The face to draw `text` (one grapheme cluster) with in `style`.
    pub(crate) fn face_for(&mut self, text: &str, style: FaceStyle) -> FaceId {
        let wanted = presentation(text);
        let own = self.primary(style);
        if wanted == Presentation::Text && self.covers(own, text) {
            return own;
        }
        let key = (text.to_owned(), wanted, style);
        if let Some(&id) = self.chosen.get(&key) {
            return id;
        }
        let found = self
            .candidates(text, wanted, style)
            .into_iter()
            .find(|&id| self.covers(id, text))
            .unwrap_or(own);
        self.chosen.insert(key, found);
        found
    }

    fn covers(&self, id: FaceId, text: &str) -> bool {
        let face = self.face(id);
        text.chars().filter(|&c| needs(c)).all(|c| face.maps(c))
    }

    /// Faces to try, best first.
    fn candidates(&mut self, text: &str, wanted: Presentation, style: FaceStyle) -> Vec<FaceId> {
        let mut families: Vec<FamilyId> = Vec::new();
        let emoji: Vec<FamilyId> = match self.lookup {
            Lookup::System => self
                .collection
                .generic_families(GenericFamily::Emoji)
                .collect(),
            Lookup::Bundled => Vec::new(),
        };
        if wanted == Presentation::Emoji {
            families.extend(&emoji);
        }
        if let (Lookup::System, Some(script)) =
            (self.lookup, text.chars().next().and_then(script_of))
        {
            families.extend(
                self.collection
                    .fallback_families(FallbackKey::new(script, None)),
            );
        }
        if self.lookup == Lookup::System {
            families.extend(self.collection.generic_families(GenericFamily::Monospace));
        }
        families.extend(self.bundled.iter().copied());
        families.extend(&emoji);
        let mut out = vec![self.primary(style), self.primary(FaceStyle::REGULAR)];
        let colour_first = wanted == Presentation::Emoji && self.lookup == Lookup::Bundled;
        for family in families {
            if let Some(id) = self.family_face(family)
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        if colour_first {
            // Bundled sets have no emoji generic family: try colour faces first.
            out.sort_by_key(|&id| self.face(id).color == ColorFormat::None);
        }
        out
    }

    /// The regular face of a fallback family, loaded once.
    fn family_face(&mut self, id: FamilyId) -> Option<FaceId> {
        if let Some(&known) = self.families.get(&id) {
            return known;
        }
        let face = self
            .collection
            .family(id)
            .and_then(|family| family.default_font().cloned())
            .and_then(|info| {
                let blob = info.load(Some(&mut self.cache))?;
                Face::load(blob, info.index(), FaceRole::Fallback)
            })
            .map(|face| self.push(face));
        self.families.insert(id, face);
        face
    }

    /// One cut of `family`, with what fontique says must be synthesised.
    fn cut(&mut self, family: &FamilyInfo, style: FaceStyle) -> Option<Face> {
        let (font_style, weight) = (
            match style.slant {
                Slant::Upright => FontStyle::Normal,
                Slant::Italic => FontStyle::Italic,
            },
            match style.weight {
                Weight::Regular => FontWeight::NORMAL,
                Weight::Bold => FontWeight::BOLD,
            },
        );
        let info = family
            .match_font(FontWidth::NORMAL, font_style, weight, true)?
            .clone();
        let synthesis = info.synthesis(FontWidth::NORMAL, font_style, weight);
        let blob = info.load(Some(&mut self.cache))?;
        let mut face = Face::load(blob, info.index(), FaceRole::Primary)?;
        face.embolden = if synthesis.embolden() {
            Embolden::Yes
        } else {
            Embolden::No
        };
        face.skew = if synthesis.skew().is_some() {
            Skew::Yes
        } else {
            Skew::No
        };
        face.variations = synthesis
            .variation_settings()
            .iter()
            .map(|(tag, value)| (u32::from_be_bytes(tag.to_be_bytes()), *value))
            .collect();
        Some(face)
    }

    fn push(&mut self, face: Face) -> FaceId {
        self.faces.push(face);
        FaceId(u16::try_from(self.faces.len() - 1).unwrap_or(u16::MAX))
    }
}

/// The chosen family: by name, else the system's monospace, else the first bundled family.
fn primary_family(
    collection: &mut Collection,
    family: &FontFamily,
    bundled: &[FamilyId],
) -> Option<FamilyInfo> {
    if let FontFamily::Named(name) = family
        && let Some(found) = collection.family_by_name(name)
    {
        return Some(found);
    }
    let generic: Vec<FamilyId> = collection
        .generic_families(GenericFamily::Monospace)
        .collect();
    generic
        .into_iter()
        .chain(bundled.iter().copied())
        .find_map(|id| collection.family(id))
}

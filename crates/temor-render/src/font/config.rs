//! What fonts the person asked for and where they come from.

use crate::metrics::{FontSize, LineHeight};

/// The grid's font family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    /// The system's `monospace` family (fontconfig's choice on Linux). The default until quire
    /// adds a terminal code-face token (PLAN §5).
    #[default]
    Monospace,
    /// A family by name; falls back to `Monospace` when it is not installed.
    Named(String),
}

/// The font settings a renderer starts with.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontConfig {
    pub family: FontFamily,
    pub size: FontSize,
    pub line_height: LineHeight,
}

/// Where fonts are looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontSource {
    /// The system's fonts, through fontique; fallbacks by emoji presentation and script.
    System,
    /// Only these font files (their bytes), in fallback order after the chosen family. Nothing
    /// on the system is read, so renders are the same on every machine (tests, golden images).
    Bundled(Vec<Vec<u8>>),
}

/// A cut of the family: weight and slant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FaceStyle {
    pub weight: Weight,
    pub slant: Slant,
}

impl FaceStyle {
    pub const REGULAR: Self = Self {
        weight: Weight::Regular,
        slant: Slant::Upright,
    };

    /// All four cuts, in the order a `FontSet` stores its primary faces.
    pub const ALL: [Self; 4] = [
        Self::REGULAR,
        Self {
            weight: Weight::Bold,
            slant: Slant::Upright,
        },
        Self {
            weight: Weight::Regular,
            slant: Slant::Italic,
        },
        Self {
            weight: Weight::Bold,
            slant: Slant::Italic,
        },
    ];

    pub fn index(self) -> usize {
        match (self.weight, self.slant) {
            (Weight::Regular, Slant::Upright) => 0,
            (Weight::Bold, Slant::Upright) => 1,
            (Weight::Regular, Slant::Italic) => 2,
            (Weight::Bold, Slant::Italic) => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Weight {
    Regular,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Slant {
    Upright,
    Italic,
}

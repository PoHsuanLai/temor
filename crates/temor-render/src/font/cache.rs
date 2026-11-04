//! `GlyphCache`: graphemes to placed glyphs, cached per style and width, at one scale. A glyph
//! is named by what it is drawn from ([`GlyphKey`]), never by where it sits in an atlas, so a run
//! handed out stays good: its images are kept here, and drawn again if they were let go. Where a
//! glyph sits in an atlas is the atlas's business (`Atlases::resolve`).

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::ScaleContext;
use swash::shape::ShapeContext;

use super::config::FaceStyle;
use super::decor::Decoration;
use super::face::{FaceId, Synth};
use super::measure::cell_metrics;
use super::set::FontSet;
use crate::metrics::{CellMetrics, Scale};
use crate::platform::SystemBitmapEmoji;

/// Which atlas a glyph lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphKind {
    /// A coverage mask, tinted with the text colour.
    Mask,
    /// Premultiplied colour, drawn as is.
    Color,
}

impl GlyphKind {
    pub fn bytes_per_texel(self) -> u32 {
        match self {
            Self::Mask => 1,
            Self::Color => 4,
        }
    }
}

/// What a glyph is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphKey {
    /// A face's glyph as a coverage mask, thickened or slanted as asked.
    Mask {
        face: FaceId,
        glyph: u16,
        synth: Synth,
    },
    /// A face's colour glyph, fitted to a box (so the box is part of what it is).
    Color {
        face: FaceId,
        glyph: u16,
        size: (u32, u32),
    },
    /// A box-drawing character or an underline, drawn here.
    Special(Special),
}

impl GlyphKey {
    pub fn kind(self) -> GlyphKind {
        match self {
            Self::Color { .. } => GlyphKind::Color,
            Self::Mask { .. } | Self::Special(_) => GlyphKind::Mask,
        }
    }
}

/// Glyphs drawn here rather than from a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Special {
    Box(char),
    Decor(Decoration),
}

/// A glyph's texels, rows packed: one coverage byte each for a mask, premultiplied RGBA for a
/// colour glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GlyphImage {
    pub width: u32,
    pub height: u32,
    pub kind: GlyphKind,
    pub data: Vec<u8>,
}

impl GlyphImage {
    /// Texel row `row`.
    pub fn row(&self, row: u32) -> &[u8] {
        let line = (self.width * self.kind.bytes_per_texel()) as usize;
        let start = row as usize * line;
        self.data.get(start..start + line).unwrap_or_default()
    }
}

/// A glyph drawn, and where it sits from the cell's top-left corner, in device pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Placed {
    pub at: (i32, i32),
    pub image: GlyphImage,
}

/// A glyph placed in a cell: `dx`, `dy` from the cell's top-left corner, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RunGlyph {
    pub dx: i32,
    pub dy: i32,
    pub w: u32,
    pub h: u32,
    pub key: GlyphKey,
}

/// Cached runs of one style and width, by grapheme.
type RunsByText = HashMap<Box<str>, Arc<[RunGlyph]>>;

/// How many bytes of glyph images are kept before they are let go (at the start of a frame).
const KEPT_BYTES: usize = 32 << 20;

/// Graphemes to placed glyphs, and the glyphs' images.
pub(crate) struct GlyphCache {
    pub(super) fonts: FontSet,
    pub(super) px: f32,
    scale: Scale,
    pub(super) metrics: CellMetrics,
    pub(super) scaler: ScaleContext,
    pub(super) shaper: ShapeContext,
    pub(super) bitmaps: SystemBitmapEmoji,
    /// Each glyph drawn; `None` for one with nothing to draw.
    drawn: HashMap<GlyphKey, Option<Placed>>,
    /// The bytes of the images in `drawn`.
    held: usize,
    runs: HashMap<(FaceStyle, u8), RunsByText>,
}

impl std::fmt::Debug for GlyphCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphCache")
            .field("metrics", &self.metrics)
            .field("held", &self.held)
            .finish_non_exhaustive()
    }
}

/// The cell `fonts` make at `scale`.
fn metrics_at(fonts: &FontSet, scale: Scale) -> CellMetrics {
    cell_metrics(
        fonts.face(fonts.primary(FaceStyle::REGULAR)),
        fonts.size().device(scale),
        fonts.line_height(),
    )
}

impl GlyphCache {
    pub fn new(fonts: FontSet, scale: Scale) -> Self {
        let px = fonts.size().device(scale);
        let metrics = metrics_at(&fonts, scale);
        Self {
            fonts,
            px,
            scale,
            metrics,
            scaler: ScaleContext::new(),
            shaper: ShapeContext::new(),
            bitmaps: SystemBitmapEmoji::new(),
            drawn: HashMap::new(),
            held: 0,
            runs: HashMap::new(),
        }
    }

    pub fn metrics(&self) -> CellMetrics {
        self.metrics
    }

    /// The cell at `scale`, whatever scale the cache is at: nothing cached changes.
    pub fn metrics_at(&self, scale: Scale) -> CellMetrics {
        metrics_at(&self.fonts, scale)
    }

    /// Moves to `scale`; everything cached is dropped when it changes, and the same keys draw
    /// other images from then on. Returns whether it did.
    pub fn set_scale(&mut self, scale: Scale) -> bool {
        if scale == self.scale {
            return false;
        }
        self.scale = scale;
        self.px = self.fonts.size().device(scale);
        self.metrics = metrics_at(&self.fonts, scale);
        self.forget();
        true
    }

    /// Lets the glyphs go when their images hold more than [`KEPT_BYTES`]; called between
    /// frames, so the images a frame's quads name are there while it draws.
    pub fn trim(&mut self) {
        if self.held > KEPT_BYTES {
            self.forget();
        }
    }

    fn forget(&mut self) {
        self.drawn.clear();
        self.runs.clear();
        self.held = 0;
    }

    /// The glyphs for one grapheme cluster `span` cells wide.
    pub fn run(&mut self, text: &str, style: FaceStyle, span: u8) -> Arc<[RunGlyph]> {
        if let Some(run) = self.runs.get(&(style, span)).and_then(|m| m.get(text)) {
            return run.clone();
        }
        let glyphs: Arc<[RunGlyph]> = self.build_run(text, style, span).into();
        self.runs
            .entry((style, span))
            .or_default()
            .insert(text.into(), glyphs.clone());
        glyphs
    }

    /// The mask for an underline style, one cell wide.
    pub fn decoration(&mut self, decoration: Decoration) -> Option<RunGlyph> {
        let key = GlyphKey::Special(Special::Decor(decoration));
        self.placed(key)
            .map(|placed| run_glyph(key, placed, (0, 0)))
    }

    /// The image of `key`, drawn now if it is not kept.
    #[cfg(any(feature = "gpu", test))]
    pub fn image_of(&mut self, key: GlyphKey) -> Option<&GlyphImage> {
        self.placed(key).map(|placed| &placed.image)
    }

    /// The image of `key` as kept: every key a run handed out since the last [`trim`] is.
    ///
    /// [`trim`]: GlyphCache::trim
    pub fn image(&self, key: GlyphKey) -> Option<&GlyphImage> {
        self.drawn.get(&key)?.as_ref().map(|placed| &placed.image)
    }

    /// `key` drawn and placed, from what is kept or drawn now.
    pub(super) fn placed(&mut self, key: GlyphKey) -> Option<&Placed> {
        if !self.drawn.contains_key(&key) {
            let placed = self.draw(key);
            self.held += placed.as_ref().map_or(0, |p| p.image.data.len());
            self.drawn.insert(key, placed);
        }
        self.drawn.get(&key)?.as_ref()
    }
}

#[cfg(test)]
impl GlyphCache {
    /// A glyph a test places by hand: a mask `w` by `h`, fully covered, under a name of its
    /// own.
    pub(crate) fn keep_mask(&mut self, w: u32, h: u32) -> GlyphKey {
        let key = TEST_GLYPH;
        let image = GlyphImage {
            width: w,
            height: h,
            kind: GlyphKind::Mask,
            data: vec![255; (w * h) as usize],
        };
        self.drawn.insert(key, Some(Placed { at: (0, 0), image }));
        key
    }
}

/// The name [`GlyphCache::keep_mask`] keeps its glyph under.
#[cfg(test)]
pub(crate) const TEST_GLYPH: GlyphKey = GlyphKey::Special(Special::Box('\u{2588}'));

/// `placed` as a run's glyph, moved by `by`.
pub(super) fn run_glyph(key: GlyphKey, placed: &Placed, by: (i32, i32)) -> RunGlyph {
    RunGlyph {
        dx: placed.at.0 + by.0,
        dy: placed.at.1 + by.1,
        w: placed.image.width,
        h: placed.image.height,
        key,
    }
}

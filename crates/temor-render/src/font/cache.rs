//! `GlyphCache`: graphemes to positioned atlas glyphs, cached per style and width, at one scale.

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::ScaleContext;
use swash::shape::ShapeContext;

use super::atlas::{Atlas, AtlasFormat, Slot};
use super::config::FaceStyle;
use super::decor::Decoration;
use super::face::{Embolden, FaceId, Skew};
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

/// A glyph placed in a cell: `dx`, `dy` from the cell's top-left corner, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RunGlyph {
    pub dx: i32,
    pub dy: i32,
    pub slot: Slot,
    pub kind: GlyphKind,
}

/// The rasterised glyphs a cache key stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RasterKey {
    pub face: FaceId,
    pub glyph: u16,
    pub synth: (Embolden, Skew),
    /// Colour glyphs are fitted to their box, so its size is part of the key.
    pub color_box: (u32, u32),
}

/// Glyphs drawn here rather than from a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Special {
    Box(char),
    Decor(Decoration),
}

/// Cached runs of one style and width, by grapheme.
type RunsByText = HashMap<Box<str>, Arc<[RunGlyph]>>;

/// Graphemes to atlas glyphs. Everything cached is valid for one [`GlyphCache::epoch`]: when an
/// atlas fills up it is cleared and the epoch moves on, and glyphs handed out before must be
/// fetched again.
pub(crate) struct GlyphCache {
    pub(super) fonts: FontSet,
    pub(super) px: f32,
    scale: Scale,
    pub(super) metrics: CellMetrics,
    pub alpha: Atlas,
    pub color: Atlas,
    pub(super) scaler: ScaleContext,
    pub(super) shaper: ShapeContext,
    pub(super) bitmaps: SystemBitmapEmoji,
    pub(super) rasters: HashMap<RasterKey, Option<RunGlyph>>,
    pub(super) specials: HashMap<Special, Option<RunGlyph>>,
    runs: HashMap<(FaceStyle, u8), RunsByText>,
    epoch: u64,
}

impl std::fmt::Debug for GlyphCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphCache")
            .field("metrics", &self.metrics)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

/// Atlas sizes: start small, grow to the cap, then clear.
const ALPHA_ATLAS: (u32, u32, u32) = (1024, 256, 4096);
const COLOR_ATLAS: (u32, u32, u32) = (1024, 256, 2048);

/// How many times a glyph run, row or frame is built again when an atlas clears while it is
/// built (its glyphs would point at cleared texels). A build that still clears after that many
/// tries is abandoned, so the work is bounded even when the glyphs cannot fit the atlas.
pub(crate) const BUILD_ATTEMPTS: usize = 3;

impl GlyphCache {
    pub fn new(fonts: FontSet, scale: Scale) -> Self {
        Self::with_max_heights(fonts, scale, ALPHA_ATLAS.2, COLOR_ATLAS.2)
    }

    /// As [`GlyphCache::new`], with the atlases capped at the given heights (texel rows). The
    /// caps are the only knob tests need to force atlas clears.
    pub(crate) fn with_max_heights(
        fonts: FontSet,
        scale: Scale,
        alpha_max: u32,
        color_max: u32,
    ) -> Self {
        let px = fonts.size().device(scale);
        let metrics = cell_metrics(fonts.face(fonts.primary(FaceStyle::REGULAR)), px);
        Self {
            fonts,
            px,
            scale,
            metrics,
            alpha: Atlas::new(AtlasFormat::Alpha, ALPHA_ATLAS.0, ALPHA_ATLAS.1, alpha_max),
            color: Atlas::new(AtlasFormat::Color, COLOR_ATLAS.0, COLOR_ATLAS.1, color_max),
            scaler: ScaleContext::new(),
            shaper: ShapeContext::new(),
            bitmaps: SystemBitmapEmoji::default(),
            rasters: HashMap::new(),
            specials: HashMap::new(),
            runs: HashMap::new(),
            epoch: 0,
        }
    }

    pub fn metrics(&self) -> CellMetrics {
        self.metrics
    }

    /// Moves to `scale`; everything cached is dropped when it changes. Returns whether it did.
    pub fn set_scale(&mut self, scale: Scale) -> bool {
        if scale == self.scale {
            return false;
        }
        self.scale = scale;
        self.px = self.fonts.size().device(scale);
        self.metrics = cell_metrics(
            self.fonts.face(self.fonts.primary(FaceStyle::REGULAR)),
            self.px,
        );
        self.reset();
        true
    }

    /// Drops the record of atlas regions written, for a renderer that reads the atlases from
    /// memory and never uploads them.
    pub fn forget_uploads(&mut self) {
        self.alpha.take_dirty();
        self.color.take_dirty();
    }

    /// Changes whenever glyphs handed out before became invalid.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The glyphs for one grapheme cluster `span` cells wide.
    pub fn run(&mut self, text: &str, style: FaceStyle, span: u8) -> Arc<[RunGlyph]> {
        if let Some(run) = self.runs.get(&(style, span)).and_then(|m| m.get(text)) {
            return run.clone();
        }
        // An atlas can fill up part-way through a run; the glyphs placed before the clear are
        // then gone, so the run is built again into the emptied atlases. A run that still
        // clears after `BUILD_ATTEMPTS` is handed out uncached: the caller sees the epoch move
        // and builds its row again.
        let mut glyphs: Arc<[RunGlyph]> = Arc::from(Vec::new());
        for _ in 0..BUILD_ATTEMPTS {
            let epoch = self.epoch;
            glyphs = self.build_run(text, style, span).into();
            if self.epoch == epoch {
                self.runs
                    .entry((style, span))
                    .or_default()
                    .insert(text.into(), glyphs.clone());
                return glyphs;
            }
        }
        glyphs
    }

    /// The mask for an underline style, one cell wide.
    pub fn decoration(&mut self, decoration: Decoration) -> Option<RunGlyph> {
        self.special(Special::Decor(decoration))
    }

    /// Drops every glyph and empties both atlases.
    pub(super) fn reset(&mut self) {
        self.alpha.clear();
        self.color.clear();
        self.rasters.clear();
        self.specials.clear();
        self.runs.clear();
        self.epoch += 1;
    }

    /// Puts a mask or colour image in its atlas, clearing everything once if it is full.
    pub(super) fn store(&mut self, kind: GlyphKind, w: u32, h: u32, data: &[u8]) -> Option<Slot> {
        if let Ok(slot) = self.atlas(kind).put(w, h, data) {
            return Some(slot);
        }
        self.reset();
        self.atlas(kind).put(w, h, data).ok()
    }

    fn atlas(&mut self, kind: GlyphKind) -> &mut Atlas {
        match kind {
            GlyphKind::Mask => &mut self.alpha,
            GlyphKind::Color => &mut self.color,
        }
    }
}

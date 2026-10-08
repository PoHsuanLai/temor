//! `GlyphCache`: graphemes to positioned atlas glyphs, cached per style and width, at one scale.

use std::collections::HashMap;
use std::sync::Arc;

use ds_core::word::Word;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
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

impl GlyphCache {
    pub fn new(fonts: FontSet, scale: Scale) -> Self {
        let px = fonts.size().device(scale);
        let metrics = cell_metrics(fonts.face(fonts.primary(FaceStyle::REGULAR)), px);
        Self {
            fonts,
            px,
            scale,
            metrics,
            alpha: Atlas::new(
                AtlasFormat::Alpha,
                ALPHA_ATLAS.0,
                ALPHA_ATLAS.1,
                ALPHA_ATLAS.2,
            ),
            color: Atlas::new(
                AtlasFormat::Color,
                COLOR_ATLAS.0,
                COLOR_ATLAS.1,
                COLOR_ATLAS.2,
            ),
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
        let epoch = self.epoch;
        let mut glyphs = self.build_run(text, style, span);
        if self.epoch != epoch {
            // An atlas filled up part-way: build again into the emptied atlases.
            glyphs = self.build_run(text, style, span);
        }
        let run: Arc<[RunGlyph]> = glyphs.into();
        self.runs
            .entry((style, span))
            .or_default()
            .insert(text.into(), run.clone());
        run
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

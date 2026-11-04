//! The glyph atlases a GPU frame draws from: shelf-packed texels in memory, growing downwards
//! (so packed slots never move) up to a limit, and where each glyph was put. A frame asks for
//! the glyphs it shows ([`Atlases::resolve`]); when they do not fit, it is told so and clears
//! them, or draws in groups that do fit. A glyph's image comes from the glyph cache, by name.

use std::collections::HashMap;

use super::cache::{GlyphCache, GlyphKey, GlyphKind};

/// A packed rectangle in an atlas, in texels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// What an atlas texel holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AtlasFormat {
    /// One coverage byte, tinted with the text colour.
    Alpha,
    /// Premultiplied RGBA.
    Color,
}

impl AtlasFormat {
    pub fn bytes_per_texel(self) -> u32 {
        match self {
            Self::Alpha => 1,
            Self::Color => 4,
        }
    }
}

/// An atlas that is out of room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AtlasFull;

/// An atlas's texels at one moment, to be drawn from after the atlas itself has been cleared.
#[cfg(feature = "gpu")]
#[derive(Debug, Clone)]
pub(crate) struct AtlasCopy {
    pub format: AtlasFormat,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// A shelf-packed atlas, plus the regions written since the GPU last copied it.
#[derive(Debug)]
pub(crate) struct Atlas {
    pub format: AtlasFormat,
    pub width: u32,
    pub height: u32,
    max_height: u32,
    pub data: Vec<u8>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    /// Regions written since the last [`Atlas::take_dirty`].
    dirty: Vec<Slot>,
    /// Bumped when the atlas grows or is cleared: a GPU copy must be rebuilt whole.
    pub generation: u64,
}

impl Atlas {
    pub fn new(format: AtlasFormat, width: u32, height: u32, max_height: u32) -> Self {
        Self {
            format,
            width,
            height,
            max_height: max_height.max(height),
            data: vec![0; (width * height * format.bytes_per_texel()) as usize],
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            dirty: Vec::new(),
            generation: 0,
        }
    }

    /// Copies `w` x `h` texels (rows packed, in this atlas's format) into a free slot.
    pub fn put(&mut self, w: u32, h: u32, src: &[u8]) -> Result<Slot, AtlasFull> {
        let bpp = self.format.bytes_per_texel();
        if src.len() < (w * h * bpp) as usize {
            return Err(AtlasFull);
        }
        let slot = self.alloc(w, h)?;
        let row = (w * bpp) as usize;
        for y in 0..h as usize {
            let dst =
                ((slot.y as usize + y) * self.width as usize + slot.x as usize) * bpp as usize;
            self.data[dst..dst + row].copy_from_slice(&src[y * row..(y + 1) * row]);
        }
        self.dirty.push(slot);
        Ok(slot)
    }

    /// The texels as they are now.
    #[cfg(feature = "gpu")]
    pub fn copy(&self) -> AtlasCopy {
        AtlasCopy {
            format: self.format,
            width: self.width,
            height: self.height,
            data: self.data.clone(),
        }
    }

    /// Empties the atlas; every slot handed out before is invalid.
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_h = 0;
        self.dirty.clear();
        self.generation += 1;
    }

    /// The regions written since the last call.
    pub fn take_dirty(&mut self) -> Vec<Slot> {
        std::mem::take(&mut self.dirty)
    }

    fn alloc(&mut self, w: u32, h: u32) -> Result<Slot, AtlasFull> {
        // One texel of gutter so sampling never bleeds between glyphs.
        let (w1, h1) = (w + 1, h + 1);
        if w1 > self.width {
            return Err(AtlasFull);
        }
        if self.shelf_x + w1 > self.width {
            self.shelf_y += self.shelf_h;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        while self.shelf_y + h1 > self.height {
            if self.height >= self.max_height {
                return Err(AtlasFull);
            }
            self.grow();
        }
        let slot = Slot {
            x: self.shelf_x,
            y: self.shelf_y,
            w,
            h,
        };
        self.shelf_x += w1;
        self.shelf_h = self.shelf_h.max(h1);
        Ok(slot)
    }

    fn grow(&mut self) {
        self.height = (self.height * 2).min(self.max_height);
        let bytes = (self.width * self.height * self.format.bytes_per_texel()) as usize;
        self.data.resize(bytes, 0);
        self.generation += 1;
    }
}

/// Atlas sizes: start small, grow to the cap.
const ALPHA_ATLAS: (u32, u32, u32) = (1024, 256, 4096);
const COLOR_ATLAS: (u32, u32, u32) = (1024, 256, 2048);

/// The glyphs asked for do not fit the atlases together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Overflow;

/// The mask and colour atlases, and where each glyph put in them is.
#[derive(Debug)]
pub(crate) struct Atlases {
    pub alpha: Atlas,
    pub color: Atlas,
    slots: HashMap<GlyphKey, Slot>,
    /// How many times they were cleared.
    clears: u64,
}

impl Default for Atlases {
    fn default() -> Self {
        Self::with_caps(ALPHA_ATLAS.2, COLOR_ATLAS.2)
    }
}

impl Atlases {
    /// Atlases that stop growing at these heights (texel rows): the knob tests use to make a
    /// frame's glyphs outgrow them.
    pub fn with_caps(alpha_max: u32, color_max: u32) -> Self {
        Self {
            alpha: Atlas::new(AtlasFormat::Alpha, ALPHA_ATLAS.0, ALPHA_ATLAS.1, alpha_max),
            color: Atlas::new(AtlasFormat::Color, COLOR_ATLAS.0, COLOR_ATLAS.1, color_max),
            slots: HashMap::new(),
            clears: 0,
        }
    }

    /// Puts each of `keys` not in yet in its atlas, its image from `glyphs`. Fails at the first
    /// that does not fit; those put before it stay.
    pub fn resolve(
        &mut self,
        keys: impl IntoIterator<Item = GlyphKey>,
        glyphs: &mut GlyphCache,
    ) -> Result<(), Overflow> {
        for key in keys {
            if self.slots.contains_key(&key) {
                continue;
            }
            let Some(image) = glyphs.image_of(key) else {
                continue;
            };
            let atlas = match key.kind() {
                GlyphKind::Mask => &mut self.alpha,
                GlyphKind::Color => &mut self.color,
            };
            let slot = atlas
                .put(image.width, image.height, &image.data)
                .map_err(|AtlasFull| Overflow)?;
            self.slots.insert(key, slot);
        }
        Ok(())
    }

    /// Where `key` is, once resolved.
    pub fn slot(&self, key: GlyphKey) -> Option<Slot> {
        self.slots.get(&key).copied()
    }

    /// Empties both atlases.
    pub fn clear(&mut self) {
        self.alpha.clear();
        self.color.clear();
        self.slots.clear();
        self.clears += 1;
    }

    /// How many times they were cleared.
    pub fn clears(&self) -> u64 {
        self.clears
    }

    /// Both atlases' texels as they are now.
    #[cfg(feature = "gpu")]
    pub fn copy(&self) -> (AtlasCopy, AtlasCopy) {
        (self.alpha.copy(), self.color.copy())
    }
}

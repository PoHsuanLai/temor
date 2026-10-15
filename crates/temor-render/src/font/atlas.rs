//! A shelf-packed glyph atlas in memory. It grows downwards (so packed slots never move) up to
//! a limit; when full, the owner clears it and every cached glyph with it.

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

    /// One texel row of `slot`, `row` texels down.
    pub fn row(&self, slot: Slot, row: u32) -> &[u8] {
        let bpp = self.format.bytes_per_texel() as usize;
        let start = ((slot.y + row) as usize * self.width as usize + slot.x as usize) * bpp;
        &self.data[start..start + slot.w as usize * bpp]
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

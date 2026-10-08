//! Kitty graphics: placements, their render geometry and borrowed pixel data.

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{
    Generation, ImageId, KittyLayer, KittyPixelFormat, PlacementId, PlacementKind, PlacementRender,
    Px, Visibility, VtError, ZIndex,
};

/// The kitty image storage of a terminal's active screen, borrowed until the terminal is next
/// written to. Read it under the terminal lock and copy or upload pixels before releasing it.
#[derive(Debug)]
pub struct KittyImages<'t> {
    term: &'t Terminal,
    raw: sys::KittyGraphics,
}

/// A stored image.
#[derive(Debug, Clone, Copy)]
pub struct KittyImage<'t> {
    pub id: ImageId,
    pub width: Px,
    pub height: Px,
    pub format: KittyPixelFormat,
    /// Changes whenever the pixels may have changed; key texture caches on it.
    pub generation: Generation,
    pub pixels: KittyPixels<'t>,
}

/// An image's pixels, or the fact that they are still arriving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyPixels<'t> {
    /// Fully decoded pixels in the image's format (RGBA for PNG and `f=32` images).
    Ready(&'t [u8]),
    Pending,
}

/// One placement of an image, with where to draw it.
#[derive(Debug, Clone, Copy)]
pub struct KittyPlacement<'t> {
    pub placement: PlacementId,
    pub kind: PlacementKind,
    pub z: ZIndex,
    pub render: PlacementRender,
    pub image: KittyImage<'t>,
}

impl Terminal {
    /// The kitty image storage, or `None` when kitty graphics are off.
    pub fn kitty(&self) -> Option<KittyImages<'_>> {
        let mut raw: sys::KittyGraphics = std::ptr::null_mut();
        // SAFETY: KITTY_GRAPHICS writes a handle borrowed until the next mutating call, which
        // `&self` rules out for `'_`.
        let code = unsafe {
            sys::ghostty_terminal_get(
                self.raw(),
                sys::TerminalData::KITTY_GRAPHICS,
                (&raw mut raw).cast(),
            )
        };
        (code == sys::Result::SUCCESS && !raw.is_null()).then_some(KittyImages { term: self, raw })
    }
}

impl<'t> KittyImages<'t> {
    /// Changes on any transmit, placement or delete; unchanged means the same placements and
    /// pixels (positions still move with scrolling).
    pub fn generation(&self) -> Generation {
        let mut g: u64 = 0;
        // SAFETY: GENERATION writes a `uint64_t`.
        unsafe {
            sys::ghostty_kitty_graphics_get(
                self.raw,
                sys::KittyGraphicsData::GENERATION,
                (&raw mut g).cast(),
            )
        };
        Generation(g)
    }

    /// The placements on `layer`, in libghostty's draw order.
    pub fn placements(
        &self,
        layer: KittyLayer,
    ) -> Result<impl Iterator<Item = KittyPlacement<'t>>, VtError> {
        let mut iter: sys::KittyGraphicsPlacementIterator = std::ptr::null_mut();
        // SAFETY: valid out-pointer; a null allocator is the default one.
        check(
            unsafe {
                sys::ghostty_kitty_graphics_placement_iterator_new(std::ptr::null(), &mut iter)
            },
            "placements",
        )?;
        let guard = IterGuard(iter);
        let layer = match layer {
            KittyLayer::All => sys::KittyPlacementLayer::ALL,
            KittyLayer::BelowBackground => sys::KittyPlacementLayer::BELOW_BG,
            KittyLayer::BelowText => sys::KittyPlacementLayer::BELOW_TEXT,
            KittyLayer::AboveText => sys::KittyPlacementLayer::ABOVE_TEXT,
        };
        // SAFETY: PLACEMENT_ITERATOR fills the iterator from this storage; LAYER takes a
        // `GhosttyKittyPlacementLayer*`.
        unsafe {
            check(
                sys::ghostty_kitty_graphics_get(
                    self.raw,
                    sys::KittyGraphicsData::PLACEMENT_ITERATOR,
                    (&raw mut iter).cast(),
                ),
                "placements",
            )?;
            sys::ghostty_kitty_graphics_placement_iterator_set(
                iter,
                sys::KittyGraphicsPlacementIteratorOption::LAYER,
                (&raw const layer).cast(),
            );
        }
        let mut out = Vec::new();
        // SAFETY: the iterator was filled from storage that stays unchanged for `'t`.
        while unsafe { sys::ghostty_kitty_graphics_placement_next(guard.0) } {
            if let Some(p) = self.placement(guard.0) {
                out.push(p);
            }
        }
        Ok(out.into_iter())
    }

    fn placement(&self, iter: sys::KittyGraphicsPlacementIterator) -> Option<KittyPlacement<'t>> {
        use sys::KittyGraphicsPlacementData as P;
        let (mut image_id, mut placement_id, mut is_virtual, mut z) = (0u32, 0u32, false, 0i32);
        // SAFETY: the iterator is positioned on a placement; each key writes its documented type.
        let image = unsafe {
            sys::ghostty_kitty_graphics_placement_get(
                iter,
                P::IMAGE_ID,
                (&raw mut image_id).cast(),
            );
            sys::ghostty_kitty_graphics_placement_get(
                iter,
                P::PLACEMENT_ID,
                (&raw mut placement_id).cast(),
            );
            sys::ghostty_kitty_graphics_placement_get(
                iter,
                P::IS_VIRTUAL,
                (&raw mut is_virtual).cast(),
            );
            sys::ghostty_kitty_graphics_placement_get(iter, P::Z, (&raw mut z).cast());
            sys::ghostty_kitty_graphics_image(self.raw, image_id)
        };
        if image.is_null() {
            return None;
        }
        let mut info = temor_vt_sys::sized!(sys::KittyGraphicsPlacementRenderInfo);
        // SAFETY: placement, image and terminal are live; `info` is a sized out-struct.
        let code = unsafe {
            sys::ghostty_kitty_graphics_placement_render_info(
                iter,
                image,
                self.term.raw(),
                &mut info,
            )
        };
        if code != sys::Result::SUCCESS {
            return None;
        }
        Some(KittyPlacement {
            placement: PlacementId(placement_id),
            kind: if is_virtual {
                PlacementKind::Virtual
            } else {
                PlacementKind::Direct
            },
            z: ZIndex(z),
            render: PlacementRender {
                pixel_width: info.pixel_width,
                pixel_height: info.pixel_height,
                cols: info.grid_cols,
                rows: info.grid_rows,
                viewport_col: info.viewport_col,
                viewport_row: info.viewport_row,
                visibility: if info.viewport_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
            },
            // SAFETY: `image` is a live handle from this storage.
            image: unsafe { read_image(image) },
        })
    }
}

/// # Safety
/// `image` must be a live image handle whose storage outlives `'t` unchanged.
unsafe fn read_image<'t>(image: sys::KittyGraphicsImage) -> KittyImage<'t> {
    use sys::KittyGraphicsImageData as I;
    let (mut id, mut width, mut height) = (0u32, 0u32, 0u32);
    let (mut format, mut generation) = (sys::KittyImageFormat::RGBA, 0u64);
    let (mut ptr, mut len): (*const u8, usize) = (std::ptr::null(), 0);
    // SAFETY: each key writes its documented type; DATA_PTR fails (leaving null) while pending.
    let pixels = unsafe {
        sys::ghostty_kitty_graphics_image_get(image, I::ID, (&raw mut id).cast());
        sys::ghostty_kitty_graphics_image_get(image, I::WIDTH, (&raw mut width).cast());
        sys::ghostty_kitty_graphics_image_get(image, I::HEIGHT, (&raw mut height).cast());
        sys::ghostty_kitty_graphics_image_get(image, I::FORMAT, (&raw mut format).cast());
        sys::ghostty_kitty_graphics_image_get(image, I::GENERATION, (&raw mut generation).cast());
        sys::ghostty_kitty_graphics_image_get(image, I::DATA_LEN, (&raw mut len).cast());
        let ready =
            sys::ghostty_kitty_graphics_image_get(image, I::DATA_PTR, (&raw mut ptr).cast());
        if ready == sys::Result::SUCCESS && !ptr.is_null() {
            // SAFETY: libghostty guarantees DATA_LEN readable bytes at DATA_PTR while the
            // storage is unchanged, which the caller guarantees for `'t`.
            KittyPixels::Ready(std::slice::from_raw_parts(ptr, len))
        } else {
            KittyPixels::Pending
        }
    };
    let format = match format {
        sys::KittyImageFormat::RGB => KittyPixelFormat::Rgb,
        sys::KittyImageFormat::GRAY_ALPHA => KittyPixelFormat::GrayAlpha,
        sys::KittyImageFormat::GRAY => KittyPixelFormat::Gray,
        _ => KittyPixelFormat::Rgba,
    };
    KittyImage {
        id: ImageId(id),
        width: Px(width),
        height: Px(height),
        format,
        generation: Generation(generation),
        pixels,
    }
}

/// Frees a placement iterator on every exit path.
struct IterGuard(sys::KittyGraphicsPlacementIterator);

impl Drop for IterGuard {
    fn drop(&mut self) {
        // SAFETY: made by `placement_iterator_new`, freed once.
        unsafe { sys::ghostty_kitty_graphics_placement_iterator_free(self.0) };
    }
}

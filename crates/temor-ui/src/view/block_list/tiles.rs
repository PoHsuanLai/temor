//! A pane's tiles: a finished block's rows drawn once each into a texture of their own, kept with
//! the pane so a repaint can draw again the ones the selection moved over without the list
//! drawing itself again.

use std::collections::HashMap;
use std::ops::Range;

use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use temor_pane::text::CellPos as ScreenCell;
use temor_render::Palette;
use temor_vt::Rgb;

use super::segments::{Placed, Seg, SegKey, Standing};
use crate::session::Session;
use crate::view::card_colours::{CardFill, card_ground_rgb};

/// What a tile was drawn with: when any of it changes the tile is drawn again.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    rows: Range<u32>,
    width: u32,
    /// The cell's size in device pixels: the font's size and the screen's scale.
    cell: (u32, u32),
    palette: Palette,
    ground: Rgb,
    /// The selection's ends where it covers the tile's rows.
    selection: Option<(ScreenCell, ScreenCell)>,
}

/// A tile's texture and what it shows.
struct Tile {
    texture: TextureHandle,
    look: Look,
}

/// The tiles of one pane's list, by segment.
#[derive(Default)]
pub struct Tiles {
    tiles: HashMap<SegKey, Tile>,
}

impl std::fmt::Debug for Tiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tiles")
            .field("count", &self.tiles.len())
            .finish()
    }
}

/// The colour a card's rows are drawn on: raised off the background, tinted when it failed, the
/// background itself while it runs (its live rows are the terminal's own); a picked card is
/// drawn as selected text is.
pub fn ground_of(palette: &Palette, standing: Standing, picked: bool) -> Rgb {
    match (picked, standing) {
        (true, _) => card_ground_rgb(palette, CardFill::Picked),
        (false, Standing::Running) => palette.background,
        (false, Standing::Failed) => card_ground_rgb(palette, CardFill::Failed),
        (false, Standing::Done) => card_ground_rgb(palette, CardFill::None),
    }
}

/// Draws `look`'s rows into `texture`.
fn draw(session: &mut Session, texture: &TextureHandle, look: &Look) {
    let _ = session.draw_rows(look.rows.clone(), Some(look.ground), |bytes, size| {
        if let Ok(pixels) = Pixels::new(
            PixelFormat::Rgba8Premultiplied,
            size.width.0,
            size.height.0,
            bytes,
        ) {
            let _ = texture.update(&pixels);
        }
    });
}

impl Tiles {
    /// Draws the tiles of `mounted` that are missing or stale, and forgets the rest.
    pub fn draw(
        &mut self,
        session: &mut Session,
        mounted: &[&Placed],
        palette: &Palette,
        new_texture: impl Fn() -> TextureHandle,
    ) {
        let (width, cell) = session.fit().map_or((0, (0, 0)), |fit| {
            (fit.pane.width.0, (fit.cell.width.0, fit.cell.height.0))
        });
        self.tiles
            .retain(|key, _| mounted.iter().any(|p| p.key == *key));
        for placed in mounted {
            let Seg::Rows {
                rows,
                standing,
                picked,
                live: false,
                ..
            } = &placed.seg
            else {
                continue;
            };
            let look = Look {
                rows: rows.clone(),
                width,
                cell,
                palette: *palette,
                ground: ground_of(palette, *standing, *picked),
                selection: session.selection_over(rows.clone()),
            };
            if self
                .tiles
                .get(&placed.key)
                .is_some_and(|tile| tile.look == look)
            {
                continue;
            }
            let texture = self
                .tiles
                .remove(&placed.key)
                .map_or_else(&new_texture, |tile| tile.texture);
            draw(session, &texture, &look);
            self.tiles.insert(placed.key, Tile { texture, look });
        }
    }

    /// Draws again the tiles whose rows the selection now covers differently.
    pub fn follow_selection(&mut self, session: &mut Session) {
        for tile in self.tiles.values_mut() {
            let selection = session.selection_over(tile.look.rows.clone());
            if selection != tile.look.selection {
                tile.look.selection = selection;
                draw(session, &tile.texture, &tile.look);
            }
        }
    }

    /// The texture of tile `key`, once drawn.
    pub fn texture(&self, key: &SegKey) -> Option<TextureHandle> {
        self.tiles.get(key).map(|tile| tile.texture.clone())
    }
}

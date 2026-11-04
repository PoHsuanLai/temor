//! `block.lens`: a plugin draws a picture for a block, and the picture shows under the block.
//!
//! The host side: which lenses a block has (the manifest's `trigger` against the block's command),
//! the pictures drawn so far, and what is sent to a lens. The block menu's item and the painting
//! under the block read from here (`Hub::block_lenses`, `Hub::lens_image_of`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ds::prelude::ImageSource;

use temor_client::model::vocab::ids::BlockId;
use temor_client::plugins::{Drawing, Plugin};

/// A lens the block menu offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensOffer {
    /// The lens's id, handed back to run it.
    pub id: String,
    /// What the menu says ("Show with Kubernetes").
    pub label: String,
}

impl LensOffer {
    /// The menu entry for `plugin`.
    pub fn of(plugin: &Plugin) -> LensOffer {
        LensOffer {
            id: plugin.id().to_owned(),
            label: format!("Show with {}", plugin.name()),
        }
    }
}

/// A picture a lens drew for a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensImage {
    /// The lens's id.
    pub plugin: String,
    /// The picture.
    pub drawing: Drawing,
    /// The picture as the page draws it (a PNG as a `data:` address), made once when it arrives.
    pub source: Arc<str>,
}

/// What the window needs to draw one block's picture: its size in device pixels and its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensPicture {
    /// The block it sits under.
    pub block: BlockId,
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// The picture as a `data:` address.
    pub source: Arc<str>,
}

/// The drawing as a PNG, or `None` when it can't be written.
fn png_of(drawing: &Drawing) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, drawing.width(), drawing.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(drawing.rgba()).ok()?;
    writer.finish().ok()?;
    Some(bytes)
}

/// The pictures drawn for one pane's blocks, and the ones still being drawn.
#[derive(Debug, Clone, Default)]
pub struct LensBook {
    images: BTreeMap<BlockId, LensImage>,
    asked: BTreeSet<BlockId>,
}

impl LensBook {
    /// A lens was asked to draw the block.
    pub fn want(&mut self, block: BlockId) {
        self.asked.insert(block);
    }

    /// A picture for the block arrived (or its lens failed): whether it was waited for. Nobody
    /// waits for one that came twice.
    pub fn arrived(&mut self, block: BlockId) -> bool {
        self.asked.remove(&block)
    }

    /// Keeps the picture `plugin` drew for the block; a later one replaces it. A picture that
    /// can't be turned into a PNG is not kept.
    pub fn put(&mut self, block: BlockId, plugin: String, drawing: Drawing) {
        let Some(bytes) = png_of(&drawing) else {
            self.images.remove(&block);
            return;
        };
        let source = Arc::from(ImageSource::png(&bytes).0);
        self.images.insert(
            block,
            LensImage {
                plugin,
                drawing,
                source,
            },
        );
    }

    /// The pictures, for drawing.
    pub fn pictures(&self) -> Vec<LensPicture> {
        self.images
            .iter()
            .map(|(block, image)| LensPicture {
                block: *block,
                width: image.drawing.width(),
                height: image.drawing.height(),
                source: Arc::clone(&image.source),
            })
            .collect()
    }

    /// The picture for the block.
    pub fn image(&self, block: BlockId) -> Option<&LensImage> {
        self.images.get(&block)
    }

    /// Takes the block's picture away (the person hid it).
    pub fn remove(&mut self, block: BlockId) {
        self.images.remove(&block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> Drawing {
        Drawing::new(1, 1, vec![0, 0, 0, 255]).expect("one pixel")
    }

    #[test]
    fn pictures_are_kept_by_block_and_a_later_one_replaces_the_last() {
        let mut book = LensBook::default();
        book.put(BlockId(1), "a".to_owned(), picture());
        book.put(BlockId(2), "a".to_owned(), picture());
        book.put(BlockId(1), "c".to_owned(), picture());
        assert_eq!(book.image(BlockId(1)).map(|i| i.plugin.as_str()), Some("c"));
        book.remove(BlockId(2));
        assert!(book.image(BlockId(2)).is_none());
        assert_eq!(book.pictures().len(), 1);
        book.want(BlockId(2));
        assert!(book.arrived(BlockId(2)));
        assert!(!book.arrived(BlockId(2)), "only once");
    }
}

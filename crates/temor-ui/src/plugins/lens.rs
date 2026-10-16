//! `block.lens`: a plugin draws a picture for a block, and the picture shows under the block.
//!
//! The host side: which lenses a block has (the manifest's `trigger` against the block's command),
//! the pictures drawn so far, and what is sent to a lens. The block menu's item and the painting
//! under the block read from here (`Hub::block_lenses`, `Hub::lens_image_of`).

use std::collections::{BTreeMap, BTreeSet};

use temor_core::vocab::ids::{BlockId, PaneId};
use temor_plugins::{Drawing, Plugin};

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
}

/// Whether a picture that arrived was asked for and is still wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    /// It was asked for.
    Yes,
    /// Nobody is waiting for it (the pane went away, or it came twice).
    No,
}

/// The pictures drawn for blocks, by pane and block, and the ones still being drawn.
#[derive(Debug, Clone, Default)]
pub struct LensBook {
    images: BTreeMap<(PaneId, BlockId), LensImage>,
    asked: BTreeSet<(PaneId, BlockId)>,
}

impl LensBook {
    /// A lens was asked to draw the block.
    pub fn want(&mut self, pane: PaneId, block: BlockId) {
        self.asked.insert((pane, block));
    }

    /// A picture for the block arrived (or its lens failed): whether it was waited for.
    pub fn arrived(&mut self, pane: PaneId, block: BlockId) -> Wanted {
        match self.asked.remove(&(pane, block)) {
            true => Wanted::Yes,
            false => Wanted::No,
        }
    }

    /// Keeps the picture `plugin` drew for the block; a later one replaces it.
    pub fn put(&mut self, pane: PaneId, block: BlockId, plugin: String, drawing: Drawing) {
        self.images
            .insert((pane, block), LensImage { plugin, drawing });
    }

    /// The picture for the block.
    pub fn image(&self, pane: PaneId, block: BlockId) -> Option<&LensImage> {
        self.images.get(&(pane, block))
    }

    /// Takes the block's picture away (the person hid it).
    pub fn remove(&mut self, pane: PaneId, block: BlockId) {
        self.images.remove(&(pane, block));
    }

    /// Forgets every picture of a pane that is gone.
    pub fn forget_pane(&mut self, pane: PaneId) {
        self.images.retain(|(owner, _), _| *owner != pane);
        self.asked.retain(|(owner, _)| *owner != pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> Drawing {
        Drawing::new(1, 1, vec![0, 0, 0, 255]).expect("one pixel")
    }

    #[test]
    fn pictures_are_kept_by_block_and_dropped_with_their_pane() {
        let (p1, p2) = (PaneId(1), PaneId(2));
        let mut book = LensBook::default();
        book.put(p1, BlockId(1), "a".to_owned(), picture());
        book.put(p1, BlockId(2), "a".to_owned(), picture());
        book.put(p2, BlockId(1), "b".to_owned(), picture());
        book.put(p1, BlockId(1), "c".to_owned(), picture());
        assert_eq!(
            book.image(p1, BlockId(1)).map(|i| i.plugin.as_str()),
            Some("c")
        );
        book.remove(p1, BlockId(2));
        assert!(book.image(p1, BlockId(2)).is_none());
        book.want(p1, BlockId(3));
        book.forget_pane(p1);
        assert!(book.image(p1, BlockId(1)).is_none());
        assert_eq!(
            book.arrived(p1, BlockId(3)),
            Wanted::No,
            "the pane went away"
        );
        assert_eq!(
            book.image(p2, BlockId(1)).map(|i| i.plugin.as_str()),
            Some("b")
        );
        book.want(p2, BlockId(2));
        assert_eq!(book.arrived(p2, BlockId(2)), Wanted::Yes);
        assert_eq!(book.arrived(p2, BlockId(2)), Wanted::No, "only once");
    }
}

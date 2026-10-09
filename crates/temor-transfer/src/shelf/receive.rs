//! Putting what arrived on the shelf: the file goes into the folder under a name nobody holds,
//! then the index records it. One path for every protocol, so a file from OSC 1337 and a file
//! from kitty end up the same.

use std::path::{Path, PathBuf};

use super::clock::Clock;
use super::model::ShelfItem;
use super::store::ShelfStore;
use crate::error::TransferError;
use crate::kitty::SavedFile;
use crate::name::FileName;
use crate::origin::Origin;
use crate::osc1337::Consented;
use crate::units::Bytes;

impl ShelfStore {
    /// Saves an allowed OSC 1337 file and records it. Only a `Consented` offer can come here.
    pub fn receive_consented(
        &self,
        offer: &Consented,
        origin: Origin,
        clock: &impl Clock,
    ) -> Result<ShelfItem, TransferError> {
        let path = self.place_consented(offer)?;
        let size = Bytes::new(offer.data().len() as u64);
        self.record(path, offer.name().clone(), size, origin, clock)
    }

    /// Records a file a kitty transfer saved under `scratch` (the transfer's own folder, which
    /// the person allowed): the file moves into the shelf's folder and is recorded. Folders in
    /// the transfer are not shelved.
    pub fn receive_saved(
        &self,
        scratch: &Path,
        saved: &SavedFile,
        origin: Origin,
        clock: &impl Clock,
    ) -> Result<ShelfItem, TransferError> {
        let source = scratch.join(saved.path.to_path_buf());
        let name = FileName::sanitise(saved.path.leaf());
        let path = self.place(&source, &name)?;
        self.record(path, name, saved.size, origin, clock)
    }

    /// Adds an item for `path` to the index and writes the index.
    fn record(
        &self,
        path: PathBuf,
        name: FileName,
        size: Bytes,
        origin: Origin,
        clock: &impl Clock,
    ) -> Result<ShelfItem, TransferError> {
        let shelf = self.load(clock)?;
        let item = ShelfItem {
            id: shelf.next_id(),
            name,
            path,
            size,
            origin,
            received_at: clock.now(),
        };
        self.save(&shelf.insert_item(item.clone()))?;
        Ok(item)
    }
}

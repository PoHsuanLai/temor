//! Content hashes for file stamps. Hashing a big file is slow, and a download asks for the same
//! file chunk after chunk, so the hash is remembered for as long as the file is unchanged.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use temor_wire::digest::ContentHash;

use crate::error::Error;
use crate::platform::RawStat;

/// How many hashes are kept before the memory is cleared.
const REMEMBERED: usize = 512;

/// SHA-256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    ContentHash(hasher.finalize().into())
}

/// What identifies a file's contents without reading them: where it is, how long and when changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    dev: u64,
    ino: u64,
    mtime: i64,
    len: u64,
}

/// Remembered hashes. `Send` and `Sync`.
#[derive(Debug, Default)]
pub(super) struct StampCache {
    known: Mutex<HashMap<Key, ContentHash>>,
}

impl StampCache {
    /// The hash of the contents `file` yields from its start, read in full unless remembered.
    /// `stat` is the file's state as it was opened.
    pub(super) fn hash(&self, file: &mut impl Read, stat: &RawStat) -> Result<ContentHash, Error> {
        let key = Key {
            dev: stat.dev,
            ino: stat.ino,
            mtime: stat.mtime.0,
            len: stat.len,
        };
        if let Some(hash) = self.lock().get(&key) {
            return Ok(*hash);
        }
        let hash = read_hash(file)?;
        let mut known = self.lock();
        if known.len() >= REMEMBERED {
            known.clear();
        }
        known.insert(key, hash);
        Ok(hash)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Key, ContentHash>> {
        self.known
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn read_hash(file: &mut impl Read) -> Result<ContentHash, Error> {
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|error| Error::io("hashing a file", error))?;
        if read == 0 {
            return Ok(ContentHash(hasher.finalize().into()));
        }
        hasher.update(&chunk[..read]);
    }
}

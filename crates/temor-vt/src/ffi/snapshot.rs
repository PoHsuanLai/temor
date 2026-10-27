//! Snapshots: a terminal's complete state as bytes, to restore later (recently closed tabs,
//! reattaching a remote session).

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::VtError;

/// An encoded terminal: screen, scrollback, styles, links, modes, cursor and title. The format
/// is libghostty's and has no compatibility promise across Ghostty pins.
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot(Vec<u8>);

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot(<{} bytes>)", self.0.len())
    }
}

impl Snapshot {
    /// A snapshot from previously stored bytes; checked when restored.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The encoded bytes, to store.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The encoded bytes, handed over to be stored.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Decodes into a fresh terminal handle with no callbacks installed.
    pub(super) fn decode(&self) -> Result<sys::Terminal, VtError> {
        let mut decoder: sys::SnapshotDecoder = std::ptr::null_mut();
        // SAFETY: the buffer outlives the decoder, which is freed on every path below.
        check(
            unsafe {
                sys::ghostty_snapshot_decoder_new_buf(
                    std::ptr::null(),
                    &mut decoder,
                    self.0.as_ptr(),
                    self.0.len(),
                )
            },
            "snapshot",
        )
        .map_err(|_| VtError::SnapshotRejected)?;
        let mut term: sys::Terminal = std::ptr::null_mut();
        // SAFETY: the decoder is live; on success `term` is a caller-owned terminal.
        let code = unsafe { sys::ghostty_snapshot_decoder_decode(decoder, &mut term) };
        // SAFETY: freed once; the decoded terminal does not borrow from it.
        unsafe { sys::ghostty_snapshot_decoder_free(decoder) };
        match (check(code, "snapshot"), term.is_null()) {
            (Ok(()), false) => Ok(term),
            (Err(VtError::OutOfMemory), _) => Err(VtError::OutOfMemory),
            _ => Err(VtError::SnapshotRejected),
        }
    }
}

impl Terminal {
    /// Encodes the whole terminal.
    pub fn snapshot(&self) -> Result<Snapshot, VtError> {
        let (mut ptr, mut len) = (std::ptr::null_mut(), 0usize);
        // SAFETY: the terminal is live; on success `ptr` holds `len` bytes from the default
        // allocator, copied and then freed with it.
        unsafe {
            check(
                sys::ghostty_snapshot_encode_alloc(
                    self.raw(),
                    std::ptr::null(),
                    &mut ptr,
                    &mut len,
                ),
                "snapshot",
            )?;
            let bytes = if ptr.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr, len).to_vec()
            };
            sys::ghostty_free(std::ptr::null(), ptr, len);
            Ok(Snapshot(bytes))
        }
    }
}

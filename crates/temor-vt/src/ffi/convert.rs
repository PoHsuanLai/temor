//! Small conversions between libghostty's C values and this crate's types.

use temor_vt_sys as sys;

use crate::model::ModeFamily;
use crate::{Rgb, TerminalMode, VtError};

/// Maps a `GhosttyResult` to `Ok` or the matching [`VtError`].
pub(super) fn check(code: sys::Result::Type, what: &'static str) -> Result<(), VtError> {
    match code {
        sys::Result::SUCCESS => Ok(()),
        sys::Result::OUT_OF_MEMORY => Err(VtError::OutOfMemory),
        sys::Result::INVALID_VALUE
        | sys::Result::OUT_OF_SPACE
        | sys::Result::NO_VALUE
        | sys::Result::LIMIT_EXCEEDED
        | sys::Result::REJECTED => Err(VtError::InvalidValue(what)),
        other => Err(VtError::Failed(other)),
    }
}

/// The packed `GhosttyMode` for a mode: the number, with bit 15 set for ANSI modes.
pub(super) fn mode_bits(mode: TerminalMode) -> sys::Mode {
    let (number, family) = mode.number();
    let ansi = match family {
        ModeFamily::Ansi => 1u16 << 15,
        ModeFamily::Dec => 0,
    };
    (number & 0x7fff) | ansi
}

pub(super) fn rgb(c: sys::ColorRgb) -> Rgb {
    Rgb::new(c.r, c.g, c.b)
}

/// Copies a borrowed C string into an owned `String`, replacing invalid UTF-8.
///
/// # Safety
/// `s.ptr` must be null or point to `s.len` readable bytes for the duration of the call.
pub(super) unsafe fn string(s: sys::String) -> String {
    String::from_utf8_lossy(unsafe { bytes(&s) }).into_owned()
}

/// Borrows a C string's bytes; a null pointer is empty.
///
/// # Safety
/// `s.ptr` must be null or point to `s.len` readable bytes that outlive the returned slice.
pub(super) unsafe fn bytes(s: &sys::String) -> &[u8] {
    if s.ptr.is_null() || s.len == 0 {
        return &[];
    }
    // SAFETY: non-null and `len` readable bytes, per this function's contract.
    unsafe { std::slice::from_raw_parts(s.ptr, s.len) }
}

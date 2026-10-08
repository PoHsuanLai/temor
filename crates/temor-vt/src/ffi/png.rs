//! The PNG decoder libghostty calls for kitty images sent as PNG (`f=100`).

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;

use temor_vt_sys as sys;

use super::callbacks::fn_ptr;

/// Installs temor's PNG decoder (the `png` crate) for kitty images, which libghostty decodes
/// inside `write`; without a decoder PNG images are dropped without a word.
///
/// The hook is **process-wide**: libghostty keeps it in one global, read by every terminal on
/// every thread. `Terminal::new` and `Terminal::restore` call this, so callers rarely need to.
/// The global is not atomic, so the store happens exactly once per process (guarded by a
/// `Once`); later calls return immediately and are safe from any thread.
pub fn install_png_decoder() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let decode: sys::SysDecodePngFn = Some(decode_png);
        // SAFETY: DECODE_PNG takes a `GhosttySysDecodePngFn`, which `decode` is bound to;
        // `decode_png` keeps no state and so is valid for the life of the process. `Once` makes
        // this the only write to libghostty's global, before any terminal can read it.
        unsafe {
            sys::ghostty_sys_set(sys::SysOption::GHOSTTY_SYS_OPT_DECODE_PNG, fn_ptr(decode));
        }
    });
}

/// Decodes `data` into 8-bit RGBA allocated with libghostty's `allocator`.
unsafe extern "C" fn decode_png(
    _userdata: *mut c_void,
    allocator: *const sys::Allocator,
    data: *const u8,
    len: usize,
    out: *mut sys::SysImage,
) -> bool {
    // A panic must not unwind into C; a failed decode just rejects the image.
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: as documented on each block in `decode_into`.
        unsafe { decode_into(allocator, data, len, out) }
    }))
    .unwrap_or(false)
}

/// # Safety
/// The arguments are those libghostty passes to the decode callback.
unsafe fn decode_into(
    allocator: *const sys::Allocator,
    data: *const u8,
    len: usize,
    out: *mut sys::SysImage,
) -> bool {
    if data.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: libghostty passes `len` readable bytes for this call.
    let input = unsafe { std::slice::from_raw_parts(data, len) };
    let Some((width, height, rgba)) = decode_rgba(input) else {
        return false;
    };
    // SAFETY: `allocator` is libghostty's, valid for this call; a null result is a refusal.
    let buf = unsafe { sys::ghostty_alloc(allocator, rgba.len()) };
    if buf.is_null() {
        return false;
    }
    // SAFETY: `buf` has `rgba.len()` writable bytes and does not overlap `rgba`; `out` is the
    // zeroed out-struct libghostty passed, which takes ownership of `buf` on success.
    unsafe {
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), buf, rgba.len());
        out.write(sys::SysImage {
            width,
            height,
            data: buf,
            data_len: rgba.len(),
        });
    }
    true
}

/// Decodes a PNG to 8-bit RGBA, or `None` if it is not a PNG this decoder handles.
pub(super) fn decode_rgba(input: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(input));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let (px, _) = buf.as_chunks::<3>();
            px.iter().flat_map(|&[r, g, b]| [r, g, b, 0xff]).collect()
        }
        png::ColorType::GrayscaleAlpha => {
            let (px, _) = buf.as_chunks::<2>();
            px.iter().flat_map(|&[g, a]| [g, g, g, a]).collect()
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 0xff]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((frame.width, frame.height, rgba))
}

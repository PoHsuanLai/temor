//! The PNG decoder libghostty calls for kitty images sent as PNG (`f=100`).

use std::ffi::c_void;

use temor_vt_sys as sys;

/// Installs temor's PNG decoder (the `png` crate) for kitty images.
///
/// **Treat the decoder as thread-local: call this on each I/O thread before its first `write`.**
/// libghostty decodes PNGs inside `write`, on the writing thread, and without a decoder PNG
/// images are dropped without a word. `Terminal::new` and `Terminal::restore` call it on the
/// creating thread; an I/O thread that writes a terminal it did not create calls it itself.
/// (libghostty's hook is currently process-wide and this decoder keeps no state, so one call
/// covers every thread today; the per-thread rule keeps callers right if that changes, as it
/// already differs in libghostty-rs.) Installing again is harmless.
pub fn install_png_decoder() {
    // SAFETY: DECODE_PNG takes a `GhosttySysDecodePngFn`; `decode_png` has that signature, keeps
    // no state and so is valid for the life of the process.
    unsafe {
        sys::ghostty_sys_set(
            sys::SysOption::GHOSTTY_SYS_OPT_DECODE_PNG,
            decode_png as *const c_void,
        );
    }
}

/// Decodes `data` into 8-bit RGBA allocated with libghostty's `allocator`.
unsafe extern "C" fn decode_png(
    _userdata: *mut c_void,
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

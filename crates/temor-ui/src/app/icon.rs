//! The window's icon, for the platforms that take it from the window (X11 and Windows). Wayland
//! and the macOS dock take theirs from the desktop entry and the bundle. The picture is the
//! 256 by 256 one the packages install (`dist/icons`), decoded once at start.

use ds::prelude::WindowIcon;

const ICON_PNG: &[u8] =
    include_bytes!("../../../../dist/icons/hicolor/256x256/apps/org.quire.Temor.png");

/// The icon, or `None` when the picture cannot be read (the window then opens with the desktop's
/// own icon).
pub fn window_icon() -> Option<WindowIcon> {
    let (width, height, rgba) = decode_rgba(ICON_PNG)?;
    WindowIcon::new(rgba, width, height).ok()
}

/// An 8-bit RGBA PNG as its size and pixels; `None` for any other kind.
fn decode_rgba(input: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(input));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());
    (frame.color_type == png::ColorType::Rgba).then_some((frame.width, frame.height, buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_packaged_picture_is_a_whole_icon() {
        let icon = window_icon().expect("the packaged icon decodes");
        assert_eq!((icon.width(), icon.height()), (256, 256));
        assert_eq!(icon.rgba().len(), 256 * 256 * 4);
    }
}

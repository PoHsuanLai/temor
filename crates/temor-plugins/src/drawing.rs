//! A lens's picture and the block it is asked about.

use crate::message::LensBlock;

/// The most output text a lens is sent: the request's JSON is at most 1 MiB, and the command and
/// the folder come with it.
pub const MOST_OUTPUT_BYTES: usize = 512 * 1024;

/// The most bytes of picture a lens may send back: 4096 by 2048 pixels of RGBA8.
pub const MOST_DRAWING_BYTES: u64 = 4096 * 2048 * 4;

/// A picture a lens drew: RGBA8, row after row, `width * height * 4` bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Drawing {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawing")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

/// Why a plugin's picture is not one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDrawing {
    /// A side of zero.
    #[error("the picture has no size")]
    Empty,
    /// The bytes are not `width * height * 4`.
    #[error("the picture says {width} by {height} but has {bytes} bytes")]
    Mismatch {
        /// The width it said.
        width: u32,
        /// The height it said.
        height: u32,
        /// The bytes it sent.
        bytes: usize,
    },
}

impl Drawing {
    /// The picture `rgba` of `width` by `height`, if the bytes are exactly that many pixels.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Drawing, BadDrawing> {
        if width == 0 || height == 0 {
            return Err(BadDrawing::Empty);
        }
        let want = u64::from(width) * u64::from(height) * 4;
        if want != rgba.len() as u64 {
            return Err(BadDrawing::Mismatch {
                width,
                height,
                bytes: rgba.len(),
            });
        }
        Ok(Drawing {
            width,
            height,
            rgba,
        })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, RGBA8.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// The request for a lens: `output` is cut at a character boundary to [`MOST_OUTPUT_BYTES`]
/// (the start is kept: a lens reads from the top).
pub fn lens_block(command: &str, output: &str, folder: &str, width: u32) -> LensBlock {
    let mut end = output.len().min(MOST_OUTPUT_BYTES);
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    LensBlock {
        command: command.to_owned(),
        output: output[..end].to_owned(),
        folder: folder.to_owned(),
        width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_has_exactly_the_bytes_its_size_says() {
        type Row = (&'static str, u32, u32, usize, Result<(), BadDrawing>);
        let table: &[Row] = &[
            ("one pixel", 1, 1, 4, Ok(())),
            ("two by three", 2, 3, 24, Ok(())),
            ("no width", 0, 3, 0, Err(BadDrawing::Empty)),
            ("no height", 3, 0, 0, Err(BadDrawing::Empty)),
            (
                "short",
                2,
                2,
                15,
                Err(BadDrawing::Mismatch {
                    width: 2,
                    height: 2,
                    bytes: 15,
                }),
            ),
            (
                "long",
                1,
                1,
                8,
                Err(BadDrawing::Mismatch {
                    width: 1,
                    height: 1,
                    bytes: 8,
                }),
            ),
        ];
        for (why, w, h, bytes, want) in table {
            let got = Drawing::new(*w, *h, vec![0; *bytes]).map(|_| ());
            assert_eq!(got, *want, "{why}");
        }
    }

    #[test]
    fn the_output_sent_to_a_lens_is_cut_on_a_character() {
        let text = "\u{e9}".repeat(MOST_OUTPUT_BYTES);
        let block = lens_block("cmd", &text, "/f", 640);
        assert!(block.output.len() <= MOST_OUTPUT_BYTES);
        assert!(block.output.chars().all(|c| c == '\u{e9}'));
        assert_eq!(lens_block("c", "short", "/f", 1).output, "short");
    }
}

//! Golden images: renders compared against PNGs in tests/golden with a small tolerance.
//! `TEMOR_UPDATE_GOLDENS=1 cargo test` rewrites them; look at every changed picture before
//! committing it.

use temor_render::PixelSize;

/// A channel may differ by this much (rounding in blends and SIMD paths)…
const CHANNEL_TOLERANCE: u8 = 3;
/// …and this share of pixels may differ by more (antialiasing at glyph edges).
const PIXEL_TOLERANCE: f64 = 0.002;

fn decode(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    (info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight).then_some((
        info.width,
        info.height,
        buf,
    ))
}

fn encode(path: &str, size: PixelSize, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size.width.0, size.height.0);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().expect("png header");
    w.write_image_data(rgba).expect("png data");
}

/// Compares `rgba` (opaque, so premultiplied equals straight) with `tests/golden/<name>.png`.
pub fn assert_golden(name: &str, size: PixelSize, rgba: &[u8]) {
    let path = format!("{}/tests/golden/{name}.png", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("TEMOR_UPDATE_GOLDENS").is_some() {
        encode(&path, size, rgba);
        return;
    }
    let actual = format!("{}/{name}.actual.png", env!("CARGO_TARGET_TMPDIR"));
    let Some((w, h, want)) = decode(&path) else {
        encode(&actual, size, rgba);
        panic!("no golden {path}; this render is at {actual} (TEMOR_UPDATE_GOLDENS=1 writes it)");
    };
    assert_eq!((w, h), (size.width.0, size.height.0), "{name}: size");
    let off = want
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgba.as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let share = off as f64 / f64::from(w * h);
    if share > PIXEL_TOLERANCE {
        encode(&actual, size, rgba);
        panic!(
            "{name}: {off} pixels differ ({:.3}%); this render is at {actual}",
            share * 100.0
        );
    }
}

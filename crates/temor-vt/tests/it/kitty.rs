//! `kitty_png.bin` (f=100, decoded by temor's PNG decoder) and `kitty_rgba.bin` (f=32): both
//! place a 2x2 image whose pixels read back exactly.

use temor_vt::{ImageId, KittyLayer, KittyPixelFormat, KittyPixels, PlacementId, PlacementIdFrom};

use crate::support::{fixture, term};

const PIXELS: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128,
];

fn placed(name: &str, id: u32) {
    let (mut t, _rx) = term(80, 24);
    let before = t.kitty().expect("kitty graphics on").generation();
    t.write(&fixture(name));
    let images = t.kitty().expect("kitty graphics on");
    assert!(
        images.generation() > before,
        "{name}: storage generation moves"
    );
    let placements: Vec<_> = images
        .placements(KittyLayer::All)
        .expect("placements")
        .collect();
    assert_eq!(placements.len(), 1, "{name}");
    let p = placements[0];
    assert_eq!(p.image.id, ImageId(id));
    assert_eq!(
        (p.image.width.0, p.image.height.0, p.image.format),
        (2, 2, KittyPixelFormat::Rgba)
    );
    assert_eq!(p.image.pixels, KittyPixels::Ready(&PIXELS), "{name}");
    assert!(p.render.visible);
    assert_eq!((p.render.viewport_col, p.render.viewport_row), (0, 0));
    assert_eq!((p.render.source_width, p.render.source_height), (2, 2));
    assert!(p.image.generation.0 > 0);
}

#[test]
fn png_image_is_decoded_and_placed() {
    placed("kitty_png.bin", 7);
}

#[test]
fn rgba_image_is_placed_byte_exact() {
    placed("kitty_rgba.bin", 9);
}

#[test]
fn layers_filter_placements() {
    let (mut t, _rx) = term(80, 24);
    t.write(&fixture("kitty_rgba.bin"));
    let images = t.kitty().expect("kitty graphics on");
    assert_eq!(
        images
            .placements(KittyLayer::AboveText)
            .expect("placements")
            .count(),
        1
    );
    assert_eq!(
        images
            .placements(KittyLayer::BelowBackground)
            .expect("placements")
            .count(),
        0
    );
}

#[test]
fn a_placement_given_no_id_is_told_from_the_programs_own() {
    let (mut t, _rx) = term(80, 24);
    // A 1x2 RGB image, placed once with no id (the terminal makes one up) and once as p=1.
    t.write(b"\x1b_Ga=t,t=d,f=24,i=1,s=1,v=2;////////\x1b\\");
    t.write(b"\x1b_Ga=p,i=1,c=10,r=1;\x1b\\");
    t.write(b"\x1b_Ga=p,i=1,p=1,c=5,r=1;\x1b\\");
    let images = t.kitty().expect("kitty graphics on");
    let mut got: Vec<(PlacementIdFrom, u32)> = images
        .placements(KittyLayer::All)
        .expect("placements")
        .map(|p| (p.id_from, p.render.cols))
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            (PlacementIdFrom::Program, 5),
            (PlacementIdFrom::Terminal, 10)
        ]
    );
    let named = images
        .placements(KittyLayer::All)
        .expect("placements")
        .find(|p| p.id_from == PlacementIdFrom::Program)
        .expect("the program's placement");
    assert_eq!(named.placement, PlacementId(1));
}

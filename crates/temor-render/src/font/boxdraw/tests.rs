use super::draw;

fn at(mask: &[u8], w: u32, x: u32, y: u32) -> u8 {
    mask[(y * w + x) as usize]
}

#[test]
fn every_character_in_the_ranges_draws_something() {
    for c in (0x2500..=0x259F).chain(0xE0B0..=0xE0B3) {
        let c = char::from_u32(c).expect("valid");
        let mask = draw(c, 10, 20, 1).unwrap_or_else(|| panic!("{c:?} not drawn"));
        assert!(mask.iter().any(|&a| a > 0), "{c:?} is empty");
    }
    assert!(draw('a', 10, 20, 1).is_none());
}

#[test]
fn horizontal_and_vertical_lines_reach_the_cell_edges() {
    let (w, h) = (10, 20);
    let horizontal = draw('─', w, h, 2).expect("drawn");
    assert_eq!(at(&horizontal, w, 0, 9), 255);
    assert_eq!(at(&horizontal, w, w - 1, 9), 255);
    assert_eq!(at(&horizontal, w, 5, 0), 0);
    let vertical = draw('│', w, h, 2).expect("drawn");
    assert_eq!(at(&vertical, w, 4, 0), 255);
    assert_eq!(at(&vertical, w, 4, h - 1), 255);
}

#[test]
fn a_double_corner_leaves_its_inside_open() {
    let (w, h) = (12, 24);
    let corner = draw('╔', w, h, 1).expect("drawn");
    // The outer stroke turns at the top-left of the band; the inner one starts lower right.
    let (x0, y0) = ((w as i32 - 3) / 2, (h as i32 - 3) / 2);
    assert_eq!(at(&corner, w, x0 as u32, y0 as u32), 255, "outer corner");
    assert_eq!(at(&corner, w, x0 as u32 + 1, y0 as u32 + 1), 0, "the gap");
    assert_eq!(
        at(&corner, w, x0 as u32 + 2, y0 as u32 + 2),
        255,
        "inner corner"
    );
    assert_eq!(at(&corner, w, x0 as u32 + 2, 0), 0, "nothing above");
}

#[test]
fn blocks_tile_their_fractions() {
    let (w, h) = (8, 16);
    let lower = draw('▄', w, h, 1).expect("drawn");
    assert_eq!(at(&lower, w, 0, 7), 0);
    assert_eq!(at(&lower, w, 0, 8), 255);
    let shade = draw('▒', w, h, 1).expect("drawn");
    assert!(shade.iter().all(|&a| a == 128));
}

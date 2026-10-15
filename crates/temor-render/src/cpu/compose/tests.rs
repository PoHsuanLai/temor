use super::*;
use crate::font::AtlasFormat;
use crate::frame::CellText;
use temor_vt::{CellStyle, Col, Selected};

const WIDTH: u32 = 8;

fn pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * WIDTH + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

#[test]
fn a_neighbours_glyph_shows_inside_the_band_and_nothing_else_of_it() {
    let mut alpha = Atlas::new(AtlasFormat::Alpha, 16, 16, 16);
    let color = Atlas::new(AtlasFormat::Color, 16, 16, 16);
    let slot = alpha.put(4, 8, &[255; 32]).expect("room");
    let white = Premul([255; 4]);
    // The row above spans y 80..100; its glyph reaches down to y 104, and its rule and a
    // background-coloured stripe lie along its own row.
    let neighbour = RowScene {
        bg: vec![Quad::solid(0, 80, WIDTH, 20, temor_vt::Rgb::new(9, 9, 9))],
        fg: vec![
            Quad {
                x: 0,
                y: 96,
                w: 4,
                h: 8,
                paint: QuadPaint::Mask { slot, color: white },
            },
            Quad {
                x: 4,
                y: 100,
                w: 4,
                h: 2,
                paint: QuadPaint::Solid(white),
            },
        ],
    };
    let mut data = vec![0; (WIDTH * 130 * 4) as usize];
    let mut target = Target {
        data: &mut data,
        width: WIDTH,
        height: 130,
        band: (100, 120),
    };
    draw_glyphs(&mut target, &alpha, &color, &neighbour);
    // (x, y, drawn): the glyph's lower part is in the band; its upper part is in the row above,
    // which is not this call's to touch; below the glyph, a rule, and a background are not drawn.
    for (x, y, drawn) in [
        (1, 100, true),
        (1, 103, true),
        (1, 104, false),
        (1, 99, false),
        (5, 100, false),
        (5, 85, false),
    ] {
        let a = pixel(&data, x, y)[3];
        assert_eq!(a == 255, drawn, "pixel ({x}, {y}) has alpha {a}");
    }
}

/// Four rows, row `i` holding `i + 1` cells, so a row can be told by its length.
fn grid() -> Vec<Vec<CellDraw>> {
    (0..4u16)
        .map(|r| {
            (0..=r)
                .map(|c| CellDraw {
                    col: Col(c),
                    span: 1,
                    text: CellText::new("a"),
                    fg: None,
                    bg: None,
                    style: CellStyle::default(),
                    selected: Selected::No,
                })
                .collect()
        })
        .collect()
}

#[test]
fn only_the_neighbours_that_may_overhang_are_taken() {
    use Overhang::{Beyond, Within};
    let rows = grid();
    // (name, row, above, below) -> lengths of the neighbours taken
    let cases = [
        (
            "none overhang: none are shaped",
            1,
            Within,
            Within,
            [None, None],
        ),
        ("the one above", 1, Beyond, Within, [Some(1), None]),
        ("the one below", 1, Within, Beyond, [None, Some(3)]),
        ("both", 1, Beyond, Beyond, [Some(1), Some(3)]),
        (
            "the top row has no row above",
            0,
            Beyond,
            Beyond,
            [None, Some(2)],
        ),
        (
            "the last row has no row below",
            3,
            Beyond,
            Beyond,
            [Some(3), None],
        ),
    ];
    for (name, at, above, below, want) in cases {
        let got = neighbour_cells(&rows, at, Sides { above, below }).map(|c| c.map(Vec::len));
        assert_eq!(got, want, "{name}");
    }
}

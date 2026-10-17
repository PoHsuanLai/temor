//! Where a lens's picture goes and how many rows it takes.

use temor_render::{
    CellMetrics, Insert, LayoutParams, Padding, Pin, PixelSize, RowLayout, Screen, fit_grid,
};
use temor_vt::{CellPx, Px, Row};

use super::{MOST_ROWS, lens_spot, request_width};
use crate::session::Gutter;

/// A gutter of 20 logical pixel rows under 4 of padding, 10 in from the sides, at `factor`, with
/// two rows of room under row `room_after` (the first rows hold on: `Pin::Top`).
fn gutter(factor: u32, room_after: u16) -> Gutter {
    let metrics = CellMetrics {
        cell: CellPx::new(10 * factor, 20 * factor),
        baseline: Px(15 * factor),
        underline: Px(16 * factor),
        strikeout: Px(10 * factor),
        thickness: Px(factor),
    };
    let fit = fit_grid(
        PixelSize::new(400 * factor, 600 * factor),
        &metrics,
        Padding::uniform(Px(4 * factor)),
    );
    let inserts = [Insert {
        after: Row(room_after),
        height: Px(40 * factor),
    }];
    Gutter {
        side: 10.0,
        top: 4.0,
        cell_h: 20.0,
        factor: factor as f32,
        layout: RowLayout::new(LayoutParams {
            fit,
            lower: 0,
            inserts: &inserts,
            pin: Pin::Top,
            screen: Screen::Primary,
        }),
    }
}

#[test]
fn a_picture_takes_whole_rows_in_the_room_under_the_blocks_last_row_and_never_grows() {
    // (name, size in device pixels, factor, last row, pane width, want (top, width, rows))
    #[rustfmt::skip]
    type Case = (&'static str, (u32, u32), u32, i64, f32, Option<(f32, f32, u32)>);
    let cases: &[Case] = &[
        (
            "small, on a plain screen",
            (100, 30),
            1,
            5,
            400.0,
            Some((124.0, 100.0, 2)),
        ),
        (
            "twice the pixels, half the size",
            (200, 60),
            2,
            5,
            400.0,
            Some((124.0, 100.0, 2)),
        ),
        (
            "exactly one row",
            (100, 20),
            1,
            0,
            400.0,
            Some((24.0, 100.0, 1)),
        ),
        (
            "wider than the pane shrinks to the room",
            (760, 100),
            1,
            2,
            400.0,
            Some((64.0, 380.0, 3)),
        ),
        (
            "too tall stops at the most rows",
            (100, 5000),
            1,
            0,
            400.0,
            Some((24.0, 100.0, MOST_ROWS)),
        ),
        (
            "a block that ended above the view has no room and no picture",
            (100, 20),
            1,
            -3,
            400.0,
            None,
        ),
    ];
    for (name, size, factor, last, pane, want) in cases {
        let room_after = u16::try_from(*last).unwrap_or(0);
        let g = gutter(*factor, room_after);
        let spot = lens_spot(*size, *last, &g, *pane);
        assert_eq!(
            spot.as_ref().map(|s| (s.top, s.width, s.rows)),
            *want,
            "{name}"
        );
        if let Some(spot) = spot {
            assert_eq!(spot.height, spot.rows as f32 * 20.0, "{name}: whole rows");
            assert_eq!(spot.left, 10.0, "{name}");
        }
    }
}

#[test]
fn the_lens_is_asked_for_the_room_between_the_paddings_in_device_pixels() {
    let g = gutter(1, 0);
    assert_eq!(request_width(400.0, &g, 1.0), 380);
    assert_eq!(request_width(400.0, &g, 2.0), 760);
    assert_eq!(
        request_width(5.0, &g, 1.0),
        1,
        "a pane with no room still asks for something"
    );
}

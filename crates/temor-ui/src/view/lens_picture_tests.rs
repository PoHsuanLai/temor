//! Where a lens's picture goes and how many rows it takes.

use super::{MOST_ROWS, lens_spot, request_width};
use crate::session::Gutter;

const GUTTER: Gutter = Gutter {
    side: 10.0,
    top: 4.0,
    cell_h: 20.0,
};

#[test]
fn a_picture_takes_whole_rows_under_the_blocks_last_row_and_never_grows() {
    // (name, size in device pixels, factor, last row, pane width, want (top, width, rows))
    #[rustfmt::skip]
    type Case = (&'static str, (u32, u32), f32, i64, f32, (f32, f32, u32));
    let cases: &[Case] = &[
        (
            "small, on a plain screen",
            (100, 30),
            1.0,
            5,
            400.0,
            (124.0, 100.0, 2),
        ),
        (
            "twice the pixels, half the size",
            (200, 60),
            2.0,
            5,
            400.0,
            (124.0, 100.0, 2),
        ),
        (
            "exactly one row",
            (100, 20),
            1.0,
            0,
            400.0,
            (24.0, 100.0, 1),
        ),
        (
            "wider than the pane shrinks to the room",
            (760, 100),
            1.0,
            2,
            400.0,
            (64.0, 380.0, 3),
        ),
        (
            "too tall stops at the most rows",
            (100, 5000),
            1.0,
            0,
            400.0,
            (24.0, 100.0, MOST_ROWS),
        ),
        (
            "a block that ended above the view starts above it",
            (100, 20),
            1.0,
            -3,
            400.0,
            (-36.0, 100.0, 1),
        ),
    ];
    for (name, size, factor, last, pane, (top, width, rows)) in cases {
        let spot = lens_spot(*size, *factor, *last, GUTTER, *pane);
        assert_eq!(
            (spot.top, spot.width, spot.rows),
            (*top, *width, *rows),
            "{name}"
        );
        assert_eq!(
            spot.height,
            *rows as f32 * GUTTER.cell_h,
            "{name}: whole rows"
        );
        assert_eq!(spot.left, GUTTER.side, "{name}");
    }
}

#[test]
fn the_lens_is_asked_for_the_room_between_the_paddings_in_device_pixels() {
    assert_eq!(request_width(400.0, GUTTER, 1.0), 380);
    assert_eq!(request_width(400.0, GUTTER, 2.0), 760);
    assert_eq!(
        request_width(5.0, GUTTER, 1.0),
        1,
        "a pane with no room still asks for something"
    );
}

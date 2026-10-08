use temor_vt::{CellPx, Col, GridSize, Px, Row, SurfacePoint};

use super::{CellSide, HitRegion, Padding, PixelSize, Scale, fit_grid};
use crate::metrics::CellMetrics;

fn metrics(w: u32, h: u32) -> CellMetrics {
    CellMetrics {
        cell: CellPx::new(w, h),
        baseline: Px(h * 3 / 4),
        underline: Px(h * 3 / 4 + 1),
        strikeout: Px(h / 2),
        thickness: Px(1),
    }
}

#[test]
fn whole_cells_fit_and_leftover_goes_right_and_bottom() {
    let fit = fit_grid(
        PixelSize::new(963, 723),
        &metrics(12, 30),
        Padding::uniform(Px(3)),
    );
    assert_eq!(fit.grid, GridSize::new(79, 23));
    assert_eq!((fit.padding.left, fit.padding.top), (Px(3), Px(3)));
    assert_eq!(fit.padding.right, Px(963 - 3 - 79 * 12));
    assert_eq!(fit.padding.bottom, Px(723 - 3 - 23 * 30));
}

#[test]
fn a_tiny_pane_still_holds_one_cell() {
    let fit = fit_grid(PixelSize::new(4, 4), &metrics(8, 16), Padding::default());
    assert_eq!(fit.grid, GridSize::new(1, 1));
    assert_eq!(fit.padding.right, Px(0));
}

#[test]
fn pixels_map_to_cells_and_back() {
    let fit = fit_grid(
        PixelSize::new(100, 100),
        &metrics(10, 20),
        Padding::uniform(Px(5)),
    );
    let hit = fit.cell_at(SurfacePoint {
        x: 5 + 23,
        y: 5 + 41,
    });
    assert_eq!((hit.pos.col, hit.pos.row), (Col(2), Row(2)));
    assert_eq!(hit.side, CellSide::Left);
    assert_eq!(hit.region, HitRegion::Grid);
    assert_eq!(fit.cell_origin(hit.pos), (Px(25), Px(45)));
    let right = fit.cell_at(SurfacePoint { x: 5 + 28, y: 5 });
    assert_eq!(right.side, CellSide::Right);
}

#[test]
fn points_outside_clamp_to_the_nearest_cell() {
    let fit = fit_grid(
        PixelSize::new(100, 100),
        &metrics(10, 20),
        Padding::uniform(Px(5)),
    );
    let hit = fit.cell_at(SurfacePoint { x: -40, y: 1000 });
    assert_eq!((hit.pos.col, hit.pos.row), (Col(0), Row(3)));
    assert_eq!(hit.region, HitRegion::Padding);
}

#[test]
fn the_mouse_surface_carries_the_padding() {
    let fit = fit_grid(
        PixelSize::new(100, 100),
        &metrics(10, 20),
        Padding::uniform(Px(5)),
    );
    let s = fit.mouse_surface();
    assert_eq!(s.padding, [Px(5), Px(5), Px(15), Px(5)]);
    assert_eq!((s.cell_width, s.cell_height), (Px(10), Px(20)));
}

#[test]
fn scale_snaps_to_device_pixels_and_rejects_nonsense() {
    assert_eq!(Scale::new(1.5).device(7.0), Px(11));
    assert_eq!(Scale::new(f32::NAN).factor(), 1.0);
    assert_eq!(Scale::new(100.0).factor(), 8.0);
    assert_eq!(
        Padding::logical(4.0, Scale::new(1.5)),
        Padding::uniform(Px(6))
    );
}

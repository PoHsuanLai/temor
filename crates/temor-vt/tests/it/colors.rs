//! Host-set default colours (and a program's OSC 10/11/4 over them), the host's selection, and
//! kitty placement pixel offsets.

use temor_vt::{
    GridPoint, KittyLayer, PointSpace, RenderView, Rgb, Row, RowFilter, SelectionShape,
    TextSelection,
};

use crate::support::{fixture, row_cells, term};

fn defaults(t: &mut temor_vt::Terminal) -> (Rgb, Rgb, Option<Rgb>, Vec<Rgb>) {
    let mut view = RenderView::new().expect("view");
    let f = view.begin(t).expect("begin").end().expect("end");
    let c = f.colors();
    (c.foreground, c.background, c.cursor, c.palette)
}

#[test]
fn default_colours_apply_and_programs_override_them() {
    let (mut t, _rx) = term(20, 4);
    let (fg, bg, cursor) = (Rgb::new(1, 2, 3), Rgb::new(4, 5, 6), Rgb::new(7, 8, 9));
    t.set_default_foreground(Some(fg)).expect("fg");
    t.set_default_background(Some(bg)).expect("bg");
    t.set_default_cursor(Some(cursor)).expect("cursor");
    assert_eq!(defaults(&mut t).0, fg);
    assert_eq!(defaults(&mut t).1, bg);
    assert_eq!(defaults(&mut t).2, Some(cursor));
    t.write(b"\x1b]11;rgb:ff/00/00\x1b\\");
    assert_eq!(defaults(&mut t).1, Rgb::new(255, 0, 0), "OSC 11 wins");
    t.write(b"\x1b]111\x1b\\");
    assert_eq!(
        defaults(&mut t).1,
        bg,
        "OSC 111 restores the host's default"
    );
}

#[test]
fn the_default_palette_colours_cells() {
    let (mut t, _rx) = term(20, 4);
    let mut palette = [Rgb::new(9, 9, 9); 256];
    palette[1] = Rgb::new(10, 20, 30);
    t.set_default_palette(Some(&palette)).expect("palette");
    t.write(b"\x1b[31mx");
    assert_eq!(row_cells(&mut t, 0)[0].fg, Some(Rgb::new(10, 20, 30)));
    assert_eq!(defaults(&mut t).3[1], Rgb::new(10, 20, 30));
    t.set_default_palette(None).expect("reset");
    assert_ne!(defaults(&mut t).3[1], Rgb::new(10, 20, 30));
}

#[test]
fn a_host_selection_marks_cells() {
    let (mut t, _rx) = term(20, 4);
    t.write(b"hello world");
    let at = |col| GridPoint::new(PointSpace::Viewport, col, 0);
    t.set_selection(Some(TextSelection {
        start: at(0),
        end: at(4),
        shape: SelectionShape::Linear,
    }))
    .expect("selection");
    {
        let mut view = RenderView::new().expect("view");
        let mut f = view.begin(&mut t).expect("begin").end().expect("end");
        let mut selected = Vec::new();
        f.rows(RowFilter::All, |r| {
            if r.row() == Row(0) {
                r.cells(|c| selected.push(c.selected)).expect("cells");
            }
        })
        .expect("rows");
        assert_eq!(&selected[..6], &[true, true, true, true, true, false]);
    }
    t.set_selection(None).expect("clear");
    assert!(
        t.set_selection(Some(TextSelection {
            start: at(0),
            end: GridPoint::new(PointSpace::Viewport, 0, 99),
            shape: SelectionShape::Block,
        }))
        .is_err()
    );
}

#[test]
fn placements_carry_their_pixel_offsets() {
    let (mut t, _rx) = term(20, 4);
    t.write(&fixture("kitty_offset.bin"));
    let images = t.kitty().expect("kitty graphics on");
    let p = images
        .placements(KittyLayer::All)
        .expect("placements")
        .next()
        .expect("one placement");
    assert_eq!((p.render.x_offset, p.render.y_offset), (3, 2));
}

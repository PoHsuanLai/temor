//! `sgr.bin`: bold/italic/underline, 256 and true colours, curly underline with colour,
//! strike/overline, faint/inverse.

use temor_vt::{Rgb, StyleFlags, Underline};

use crate::support::{fixture, row_cells, term};

#[test]
fn sgr_attributes_and_colours_resolve_per_cell() {
    let (mut t, _rx) = term(40, 4);
    t.write(&fixture("sgr.bin"));
    let cells = row_cells(&mut t, 0);
    let at = |text: &str| {
        cells
            .iter()
            .find(|c| c.text == text)
            .unwrap_or_else(|| panic!("no cell {text}"))
    };

    let a = at("A");
    assert!(
        a.style
            .flags
            .contains(StyleFlags::BOLD.with(StyleFlags::ITALIC))
    );
    assert_eq!(a.style.underline, Underline::Single);
    assert_eq!(a.fg, None, "A keeps the default foreground");

    // xterm palette entry 196 is pure red.
    assert_eq!(at("B").fg, Some(Rgb::new(255, 0, 0)));

    let c = at("C");
    assert_eq!(
        (c.fg, c.bg),
        (Some(Rgb::new(10, 20, 30)), Some(Rgb::new(1, 2, 3)))
    );

    let d = at("D");
    assert_eq!(d.style.underline, Underline::Curly);
    assert_eq!(d.style.underline_color, Some(Rgb::new(9, 8, 7)));

    assert!(
        at("E")
            .style
            .flags
            .contains(StyleFlags::STRIKETHROUGH.with(StyleFlags::OVERLINE))
    );
    assert!(
        at("F")
            .style
            .flags
            .contains(StyleFlags::FAINT.with(StyleFlags::INVERSE))
    );

    let plain = cells
        .iter()
        .find(|c| c.text.is_empty())
        .expect("a blank cell");
    assert_eq!(plain.style, Default::default());
}

use temor_vt::{Col, ColorRef, Generation, ImageId, PlacementIdFrom, Rgb};

use super::super::diacritics::mark;
use super::super::placeholder::PLACEHOLDER;
use super::*;

/// A run's slice of the fitted image: the source rectangle, the drawn size and the offset.
type Fitted = ([u32; 4], (u32, u32), (u32, u32));

fn run(col: u16, width: u32, row: u32, column: u32) -> PlaceholderRun {
    PlaceholderRun {
        col: Col(col),
        width,
        image: ImageId(7),
        placement: PlacementId(0),
        row,
        column,
    }
}

fn fit_of(
    run: PlaceholderRun,
    grid: (u32, u32),
    image: (u32, u32),
    cell: (u32, u32),
) -> Option<Fitted> {
    fit(&run, grid, image, cell).map(|f| (f.source, f.size, f.offset))
}

#[test]
fn a_run_takes_its_slice_of_the_fitted_image() {
    // Cells are 10 x 20 pixels. (name, run, grid, image) -> (source, drawn size, offset)
    let cases = [
        (
            "an image that fills the grid: the second column is its right half",
            run(0, 1, 0, 1),
            (2, 2),
            (20, 40),
            Some(([10, 0, 10, 20], (10, 20), (0, 0))),
        ),
        (
            "a run two cells wide takes two cells of source",
            run(0, 2, 1, 0),
            (2, 2),
            (20, 40),
            Some(([0, 20, 20, 20], (20, 20), (0, 0))),
        ),
        (
            "a wide image is centred: the top cell starts below the padding",
            run(0, 1, 0, 0),
            (2, 2),
            (40, 20),
            Some(([0, 0, 20, 10], (10, 5), (0, 15))),
        ),
        (
            "and the bottom cell stops at the image's end",
            run(0, 1, 1, 0),
            (2, 2),
            (40, 20),
            Some(([0, 10, 20, 10], (10, 5), (0, 0))),
        ),
        (
            "a tall image is centred sideways",
            run(0, 1, 0, 0),
            (2, 2),
            (20, 80),
            Some(([0, 0, 10, 40], (5, 20), (5, 0))),
        ),
        (
            "an empty image has no slice",
            run(0, 1, 0, 0),
            (2, 2),
            (0, 40),
            None,
        ),
        (
            "a grid of no cells has none",
            run(0, 1, 0, 0),
            (0, 2),
            (20, 40),
            None,
        ),
    ];
    for (name, run, grid, image, want) in cases {
        assert_eq!(fit_of(run, grid, image, (10, 20)), want, "{name}");
    }
}

fn target(layer: KittyLayer, placement: u32, cols: u32, rows: u32) -> VirtualTarget {
    VirtualTarget {
        layer,
        image: Arc::new(ImagePixels {
            id: ImageId(7),
            generation: Generation(1),
            width: 20,
            height: 40,
            rgba: vec![255; 20 * 40 * 4],
        }),
        placement: PlacementId(placement),
        id_from: PlacementIdFrom::Program,
        cols,
        rows,
    }
}

fn cell(col: u16, marks: &[usize], underline: ColorRef) -> PlaceholderCell {
    let text: String = std::iter::once(PLACEHOLDER)
        .chain(marks.iter().map(|&m| mark(m)))
        .collect();
    let fg = ColorRef::Rgb(Rgb::new(0, 0, 7));
    PlaceholderCell::decode(Col(col), &text, fg, underline).expect("a placeholder")
}

#[test]
fn a_placeholder_cell_yields_its_slice_of_the_image() {
    let scene = VirtualScene {
        targets: vec![
            target(KittyLayer::AboveText, 3, 2, 2),
            target(KittyLayer::BelowText, 4, 1, 1),
        ],
        cell: (10, 20),
    };
    // Row 0 is empty; row 1 holds the image's second row, both columns; row 2 names the
    // second placement by its underline colour.
    let rows = vec![
        vec![],
        vec![
            cell(2, &[1, 0], ColorRef::Palette(3)),
            cell(3, &[], ColorRef::Palette(3)),
        ],
        vec![cell(0, &[0, 0], ColorRef::Palette(4))],
    ];
    let got: Vec<_> = virtual_images(&rows, &scene)
        .into_iter()
        .map(|p| {
            (
                p.layer,
                p.col,
                p.row,
                p.source,
                (p.width.0, p.height.0),
                p.image.id,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (
                KittyLayer::AboveText,
                2,
                1,
                [0, 20, 20, 20],
                (20, 20),
                ImageId(7)
            ),
            (
                KittyLayer::BelowText,
                0,
                2,
                [0, 0, 20, 40],
                (10, 20),
                ImageId(7)
            ),
        ]
    );
}

#[test]
fn a_placeholder_naming_no_placement_or_image_draws_nothing() {
    let scene = VirtualScene {
        targets: vec![target(KittyLayer::AboveText, 3, 2, 2)],
        cell: (10, 20),
    };
    // (placement named by the underline colour) -> placements drawn: the exact one only, or
    // the image's first when none is named.
    for (underline, drawn) in [
        (ColorRef::Unset, 1),
        (ColorRef::Palette(3), 1),
        (ColorRef::Palette(9), 0),
    ] {
        let rows = vec![vec![cell(0, &[0, 0], underline)]];
        assert_eq!(virtual_images(&rows, &scene).len(), drawn);
    }
    let none = VirtualScene::default();
    assert!(virtual_images(&[vec![cell(0, &[], ColorRef::Unset)]], &none).is_empty());
}

#[test]
fn a_placeholder_names_the_programs_placement_and_never_the_terminals() {
    // The program's placements draw above text and the terminal's (made up for placements
    // given no id) below, so the layer tells which one a run met. The terminal's 3 and the
    // program's 3 are different placements.
    let program = |placement| target(KittyLayer::AboveText, placement, 2, 2);
    let terminal = |placement| VirtualTarget {
        id_from: PlacementIdFrom::Terminal,
        ..target(KittyLayer::BelowText, placement, 2, 2)
    };
    // (name, placements, the run's underline colour) -> the layer drawn, or nothing
    let cases = [
        (
            "the program's 3, not the terminal's",
            vec![terminal(3), program(3)],
            ColorRef::Palette(3),
            Some(KittyLayer::AboveText),
        ),
        (
            "only the terminal's 3: nothing",
            vec![terminal(3)],
            ColorRef::Palette(3),
            None,
        ),
        (
            "no id named: the program's placement before the terminal's",
            vec![terminal(1), program(4)],
            ColorRef::Unset,
            Some(KittyLayer::AboveText),
        ),
        (
            "no id named and only the terminal's: drawn",
            vec![terminal(2)],
            ColorRef::Unset,
            Some(KittyLayer::BelowText),
        ),
    ];
    for (name, targets, underline, want) in cases {
        let scene = VirtualScene {
            targets,
            cell: (10, 20),
        };
        let rows = vec![vec![cell(0, &[0, 0], underline)]];
        let got = virtual_images(&rows, &scene).first().map(|p| p.layer);
        assert_eq!(got, want, "{name}");
    }
}

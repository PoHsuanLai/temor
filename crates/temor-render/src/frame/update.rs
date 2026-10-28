//! Applying a captured frame to a [`PaneFrame`] all at once, so a capture that fails half way
//! leaves the frame as it was (the old size and rows), and the next capture reads everything
//! again.

use temor_vt::{CursorView, GridSize, Row, VtError};

use super::model::{CellDraw, ImagePlacement, PaneFrame, TermColors};
use super::placeholder::PlaceholderCell;
use super::virtual_images::{VirtualScene, virtual_images};

/// What one capture read besides the rows.
#[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
pub(crate) struct Update {
    pub size: GridSize,
    pub colors: TermColors,
    pub cursor: Option<CursorView>,
    pub images: Vec<ImagePlacement>,
    /// The placeholder cells of the rows that were read (the others keep theirs).
    pub placeholders: Vec<(Row, Vec<PlaceholderCell>)>,
    /// The virtual placements those cells are meant to show.
    pub virtuals: VirtualScene,
}

impl PaneFrame {
    /// Applies `update` and its `rows` (the first row that failed to read is the error). On an
    /// error nothing changes: not the size, not the colours, not a single row.
    #[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
    pub(crate) fn apply(
        &mut self,
        update: Update,
        rows: Vec<Result<(Row, Vec<CellDraw>), VtError>>,
    ) -> Result<(), VtError> {
        let rows = rows.into_iter().collect::<Result<Vec<_>, _>>()?;
        self.resize(update.size);
        self.set_colors(update.colors);
        self.cursor = update.cursor;
        for (row, cells) in rows {
            self.set_row(row, cells);
        }
        for (row, cells) in update.placeholders {
            self.set_placeholders(row, cells);
        }
        self.images = update.images;
        self.images
            .extend(virtual_images(&self.placeholders, &update.virtuals));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::model::CellText;
    use crate::frame::model::ImagePixels;
    use crate::frame::virtual_images::VirtualTarget;
    use std::sync::Arc;
    use temor_vt::{CellStyle, Col, ColorRef, Generation, ImageId, KittyLayer, PlacementId};

    fn cell(c: char) -> CellDraw {
        CellDraw {
            col: Col(0),
            span: 1,
            text: CellText::new(&c.to_string()),
            fg: None,
            bg: None,
            style: CellStyle::default(),
            selected: false,
        }
    }

    fn update(cols: u16, rows: u16) -> Update {
        Update {
            size: GridSize::new(cols, rows),
            colors: TermColors::default(),
            cursor: None,
            images: Vec::new(),
            placeholders: Vec::new(),
            virtuals: VirtualScene::default(),
        }
    }

    #[test]
    fn a_capture_that_fails_after_a_resize_leaves_the_old_size_and_rows() {
        let mut frame = PaneFrame::new();
        frame
            .apply(update(4, 2), vec![Ok((Row(0), vec![cell('a')]))])
            .expect("first capture");
        let _ = frame.take_dirty();
        let before = frame.clone();

        // The grid grew, and the second row could not be read.
        let failed = frame.apply(
            update(6, 3),
            vec![
                Ok((Row(0), vec![cell('b')])),
                Err(VtError::InvalidValue("row")),
            ],
        );
        assert!(failed.is_err());
        assert_eq!(frame, before, "nothing of the failed capture was applied");

        // A retry at the new size applies in full, and every row is dirty because the size moved.
        frame
            .apply(update(6, 3), vec![Ok((Row(0), vec![cell('b')]))])
            .expect("retry");
        assert_eq!(frame.size(), GridSize::new(6, 3));
        assert!(frame.dirty.iter().all(|d| *d));
    }

    #[test]
    fn the_blank_rows_below_are_those_under_the_text_and_the_cursor() {
        let mut frame = PaneFrame::new();
        assert_eq!(frame.blank_rows_below(), 0, "an empty frame");
        frame
            .apply(update(4, 10), vec![Ok((Row(2), vec![cell('a')]))])
            .expect("capture");
        // Text on row 2 of 10: rows 3 to 9 are empty.
        assert_eq!(frame.blank_rows_below(), 7);
        // The cursor lower than the text holds its row.
        let mut at = update(4, 10);
        at.cursor = Some(temor_vt::CursorView {
            col: Col(0),
            row: Row(5),
            shape: temor_vt::CursorShape::Block,
            blinking: false,
            wide_tail: false,
        });
        frame
            .apply(at, vec![Ok((Row(2), vec![cell('a')]))])
            .expect("capture");
        assert_eq!(frame.blank_rows_below(), 4);
        // A coloured blank row is drawn.
        let mut blank = cell(' ');
        blank.bg = Some(temor_vt::Rgb::new(1, 2, 3));
        frame
            .apply(update(4, 10), vec![Ok((Row(8), vec![blank]))])
            .expect("capture");
        assert_eq!(frame.blank_rows_below(), 1);
    }

    /// A 20 x 40 image on a 2 x 2 grid of 10 x 20 pixel cells.
    fn scene() -> VirtualScene {
        VirtualScene {
            targets: vec![VirtualTarget {
                layer: KittyLayer::AboveText,
                image: Arc::new(ImagePixels {
                    id: ImageId(7),
                    generation: Generation(1),
                    width: 20,
                    height: 40,
                    rgba: vec![255; 20 * 40 * 4],
                }),
                placement: PlacementId(1),
                id_from: temor_vt::PlacementIdFrom::Terminal,
                cols: 2,
                rows: 2,
            }],
            cell: (10, 20),
        }
    }

    fn placeholder(col: u16) -> PlaceholderCell {
        let fg = ColorRef::Rgb(temor_vt::Rgb::new(0, 0, 7));
        PlaceholderCell::decode(
            Col(col),
            &super::super::placeholder::PLACEHOLDER.to_string(),
            fg,
            ColorRef::Unset,
        )
        .expect("a placeholder")
    }

    #[test]
    fn placeholder_cells_show_their_image_until_their_row_is_read_again() {
        let mut frame = PaneFrame::new();
        let mut first = update(4, 3);
        first.placeholders = vec![(Row(1), vec![placeholder(0)])];
        first.virtuals = scene();
        frame
            .apply(first, vec![Ok((Row(1), Vec::new()))])
            .expect("capture");
        let seen = |frame: &PaneFrame| -> Vec<(i32, i32, [u32; 4])> {
            frame
                .images
                .iter()
                .map(|p| (p.col, p.row, p.source))
                .collect()
        };
        // The first cell with no marks is the image's top-left slice, on the cell's own row.
        assert_eq!(seen(&frame), vec![(0, 1, [0, 0, 10, 20])]);

        // A capture that read only another row keeps the cells of the rows it did not read.
        let mut other = update(4, 3);
        other.virtuals = scene();
        frame
            .apply(other, vec![Ok((Row(0), vec![cell('a')]))])
            .expect("capture");
        assert_eq!(seen(&frame).len(), 1);

        // Reading the row again without the cell takes the image away.
        let mut gone = update(4, 3);
        gone.virtuals = scene();
        frame
            .apply(gone, vec![Ok((Row(1), vec![cell('b')]))])
            .expect("capture");
        assert!(seen(&frame).is_empty());
    }
}

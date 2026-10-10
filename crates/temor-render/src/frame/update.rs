//! Applying a captured frame to a [`PaneFrame`] all at once, so a capture that fails half way
//! leaves the frame as it was (the old size and rows), and the next capture reads everything
//! again.

use temor_vt::{CursorView, GridSize, Row, VtError};

use super::model::{CellDraw, ImagePlacement, PaneFrame, TermColors};

/// What one capture read besides the rows.
#[cfg_attr(not(feature = "ghostty"), allow(dead_code))]
pub(crate) struct Update {
    pub size: GridSize,
    pub colors: TermColors,
    pub cursor: Option<CursorView>,
    pub images: Vec<ImagePlacement>,
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
        self.images = update.images;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::model::CellText;
    use temor_vt::{CellStyle, Col, RowDirty, Selected};

    fn cell(c: char) -> CellDraw {
        CellDraw {
            col: Col(0),
            span: 1,
            text: CellText::new(&c.to_string()),
            fg: None,
            bg: None,
            style: CellStyle::default(),
            selected: Selected::No,
        }
    }

    fn update(cols: u16, rows: u16) -> Update {
        Update {
            size: GridSize::new(cols, rows),
            colors: TermColors::default(),
            cursor: None,
            images: Vec::new(),
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
        assert!(frame.dirty.iter().all(|d| *d == RowDirty::Dirty));
    }
}

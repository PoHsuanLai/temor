//! Where and how the cursor is drawn this frame.

use ds_core::word::Word;
use temor_vt::{Col, CursorCell, CursorShape, CursorView, Rgb, Row};

use crate::frame::{CellDraw, TermColors};
use crate::palette::Palette;
use crate::renderer::PaneFocus;

/// The drawn form of the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(crate) enum CursorStyle {
    Block,
    Bar,
    Underline,
    Hollow,
}

/// The cursor resolved for drawing: its first column, width in cells, form and colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CursorDraw {
    pub col: Col,
    pub row: Row,
    pub span: u8,
    pub style: CursorStyle,
    pub color: Rgb,
}

impl CursorDraw {
    /// Whether the cursor covers column `col` of its row.
    pub fn covers(&self, col: Col) -> bool {
        col.0 >= self.col.0 && u32::from(col.0) < u32::from(self.col.0) + u32::from(self.span)
    }
}

/// The cursor to draw, if it is shown. On the right half of a wide character it moves to the
/// character's first column; on a wide character it spans both. Unfocused, every shape becomes
/// the hollow block.
pub(crate) fn resolve_cursor(
    view: Option<CursorView>,
    row_cells: impl Fn(Row) -> Option<Vec<(Col, u8)>>,
    focus: PaneFocus,
    palette: &Palette,
    colors: &TermColors,
) -> Option<CursorDraw> {
    let view = view?;
    let col = match view.cell {
        CursorCell::WideTail => Col(view.col.0.saturating_sub(1)),
        CursorCell::Normal => view.col,
    };
    let span = row_cells(view.row)
        .and_then(|cells| cells.into_iter().find(|(c, _)| *c == col).map(|(_, s)| s))
        .unwrap_or(1);
    let style = match (focus, view.shape) {
        (PaneFocus::Unfocused, _) | (PaneFocus::Focused, CursorShape::BlockHollow) => {
            CursorStyle::Hollow
        }
        (PaneFocus::Focused, CursorShape::Block) => CursorStyle::Block,
        (PaneFocus::Focused, CursorShape::Bar) => CursorStyle::Bar,
        (PaneFocus::Focused, CursorShape::Underline) => CursorStyle::Underline,
    };
    Some(CursorDraw {
        col,
        row: view.row,
        span,
        style,
        color: colors.cursor.unwrap_or(palette.cursor),
    })
}

/// The (column, span) of each drawn cell in a row, for `resolve_cursor`.
pub(crate) fn spans(cells: &[CellDraw]) -> Vec<(Col, u8)> {
    cells.iter().map(|c| (c.col, c.span)).collect()
}

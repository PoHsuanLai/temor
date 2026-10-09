//! One screen row as the pane reads it for copying and searching: one entry per cell column, so
//! a wide character (two columns) is one cell followed by a spacer. Pure.

/// How many columns a cell covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Span {
    /// One column.
    Single,
    /// The first column of a wide character; the next cell is its spacer.
    Double,
    /// The second column of a wide character. Holds no text of its own.
    Spacer,
}

impl Span {
    /// The columns this cell occupies in the row (a spacer's are its wide character's).
    pub fn columns(self) -> u16 {
        match self {
            Span::Single => 1,
            Span::Double => 2,
            Span::Spacer => 0,
        }
    }
}

/// One cell: its text (empty when blank) and its width.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowCell {
    pub text: String,
    pub span: Span,
}

/// One screen row, cell by cell, and whether it continues on the next row (a soft wrap).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowText {
    pub cells: Vec<RowCell>,
    pub wraps: bool,
}

impl RowText {
    /// A row of single-width cells, one per character (for plain text and tests).
    pub fn plain(text: &str, wraps: bool) -> Self {
        Self {
            cells: text
                .chars()
                .map(|c| RowCell {
                    text: c.to_string(),
                    span: Span::Single,
                })
                .collect(),
            wraps,
        }
    }

    /// The cell at `col`, if the row has one.
    pub fn cell(&self, col: usize) -> Option<&RowCell> {
        self.cells.get(col)
    }

    /// The text of the whole row (spacers add nothing).
    pub fn text(&self) -> String {
        self.cells.iter().map(|c| c.text.as_str()).collect()
    }

    /// The class of the cell at `col` for word selection: a spacer belongs with the wide
    /// character before it.
    pub fn first_char_at(&self, col: usize) -> Option<char> {
        let mut at = col;
        loop {
            let cell = self.cells.get(at)?;
            if cell.span != Span::Spacer {
                return cell.text.chars().next();
            }
            at = at.checked_sub(1)?;
        }
    }
}

//! The selection model (PLAN §6): an anchor and a head in screen rows and columns, extended by
//! drag, snapped to words or lines, linear or rectangular. Columns are cell columns: a wide
//! character covers two. Pure: it reads a row only through the `RowText` it is handed.

use ds_core::word::Word;

use super::row::{RowText, Span, Wrap};

/// A cell in the screen space: row 0 is the oldest kept line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellPos {
    pub row: u32,
    pub col: u16,
}

/// How a press grows the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Granularity {
    /// Single cells (a drag).
    Cells,
    /// Whole words (a double-click).
    Words,
    /// Whole lines (a triple-click).
    Lines,
}

/// Which cells a selection covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Shape {
    /// Reading order from the start to the end.
    Linear,
    /// The rectangle between the two corners (Alt-drag).
    Block,
}

/// A selection in progress or made. Each end keeps the span its granularity snapped it to, so a
/// word or line selection grows whole in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    anchor: Endpoint,
    head: Endpoint,
    granularity: Granularity,
    shape: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Endpoint {
    pos: CellPos,
    /// Inclusive columns this end covers on its row.
    span: (u16, u16),
}

impl Selection {
    /// A selection that starts at `at`, on `line` (the text of that row), growing by
    /// `granularity`.
    pub fn begin(at: CellPos, line: &RowText, granularity: Granularity, shape: Shape) -> Self {
        let end = Endpoint::at(at, line, granularity);
        Self {
            anchor: end,
            head: end,
            granularity,
            shape,
        }
    }

    /// Moves the head to `at`, on `line`, snapping to whole words or lines.
    pub fn extend(&mut self, at: CellPos, line: &RowText) {
        self.head = Endpoint::at(at, line, self.granularity);
    }

    /// The two ends in reading order, both included (each end's span widened to its word or
    /// line).
    pub fn ends(&self) -> (CellPos, CellPos) {
        let (first, last) = if self.anchor.pos <= self.head.pos {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        (first.start(), last.end())
    }

    /// The shape of the selection.
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// Whether the cell at `pos` is selected.
    pub fn contains(&self, pos: CellPos) -> bool {
        let (start, end) = self.ends();
        match self.shape {
            Shape::Linear => start <= pos && pos <= end,
            Shape::Block => {
                let (c0, c1) = (start.col.min(end.col), start.col.max(end.col));
                start.row <= pos.row && pos.row <= end.row && c0 <= pos.col && pos.col <= c1
            }
        }
    }

    /// The selected text. `line_of` gives a screen row. Soft-wrapped rows join the next row
    /// without a line break; other rows are separated by `\n`. Each row contributes its cells
    /// between the two ends; a hard-broken row's trailing blanks are dropped.
    pub fn text(&self, line_of: impl Fn(u32) -> RowText) -> String {
        let (start, end) = self.ends();
        let mut out = String::new();
        let mut previous_wraps = false;
        for row in start.row..=end.row {
            let line = line_of(row);
            let (from, to) = match self.shape {
                Shape::Linear => {
                    let from = if row == start.row {
                        start.col as usize
                    } else {
                        0
                    };
                    let to = if row == end.row {
                        end.col as usize + 1
                    } else {
                        line.cells.len()
                    };
                    (from, to)
                }
                Shape::Block => {
                    let (c0, c1) = (start.col.min(end.col), start.col.max(end.col));
                    (c0 as usize, c1 as usize + 1)
                }
            };
            let from = from.min(line.cells.len());
            let to = to.min(line.cells.len()).max(from);
            let piece: String = line.cells[from..to]
                .iter()
                .map(|c| c.text.as_str())
                .collect();
            let joins = row != start.row && previous_wraps && self.shape == Shape::Linear;
            if row != start.row && !joins {
                out.push('\n');
            }
            match line.wrap == Wrap::Continues && self.shape == Shape::Linear {
                true => out.push_str(&piece),
                false => out.push_str(piece.trim_end()),
            }
            previous_wraps = line.wrap == Wrap::Continues;
        }
        out
    }
}

impl Endpoint {
    fn at(pos: CellPos, line: &RowText, granularity: Granularity) -> Self {
        Self {
            pos,
            span: snap(granularity, line, pos.col),
        }
    }

    fn start(self) -> CellPos {
        CellPos {
            row: self.pos.row,
            col: self.span.0,
        }
    }

    fn end(self) -> CellPos {
        CellPos {
            row: self.pos.row,
            col: self.span.1,
        }
    }
}

impl CellPos {
    pub fn new(row: u32, col: u16) -> Self {
        Self { row, col }
    }
}

/// The inclusive column span a granularity covers at `col` on `line`.
pub fn snap(granularity: Granularity, line: &RowText, col: u16) -> (u16, u16) {
    match granularity {
        Granularity::Cells => (col, col),
        Granularity::Words => word_span(line, col),
        Granularity::Lines => {
            let last = line.cells.len().saturating_sub(1);
            (0, u16::try_from(last).unwrap_or(u16::MAX))
        }
    }
}

/// The run of one character class around `col` (in cells): word characters (letters, digits and
/// `_`), blanks, or anything else. A wide character's spacer is part of the character before it.
/// Past the end of the row, the cell itself.
pub fn word_span(line: &RowText, col: u16) -> (u16, u16) {
    let at = col as usize;
    if at >= line.cells.len() {
        return (col, col);
    }
    let class = line.first_char_at(at).map(class_of).unwrap_or(Class::Blank);
    let same = |i: usize| {
        line.cells[i].span == Span::Spacer
            || line.first_char_at(i).map(class_of).unwrap_or(Class::Blank) == class
    };
    let mut from = at;
    while from > 0 && same(from - 1) {
        from -= 1;
    }
    let mut to = at;
    while to + 1 < line.cells.len() && same(to + 1) {
        to += 1;
    }
    (from as u16, to as u16)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Blank,
    Other,
}

fn class_of(c: char) -> Class {
    if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else if c.is_whitespace() {
        Class::Blank
    } else {
        Class::Other
    }
}

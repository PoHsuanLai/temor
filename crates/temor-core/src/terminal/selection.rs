//! The selection model (PLAN §6): an anchor and a head in screen rows and columns, extended by
//! drag, snapped to words or lines, linear or rectangular. Pure: it reads the text of a line
//! only through the `&str` it is handed. Columns count characters, not cells (wide glyphs are a
//! known gap, noted in the app's report).

use ds_core::word::Word;

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
    pub fn begin(at: CellPos, line: &str, granularity: Granularity, shape: Shape) -> Self {
        let end = Endpoint::at(at, line, granularity);
        Self {
            anchor: end,
            head: end,
            granularity,
            shape,
        }
    }

    /// Moves the head to `at`, on `line`, snapping to whole words or lines.
    pub fn extend(&mut self, at: CellPos, line: &str) {
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

    /// The selected text. `line_of` gives the text of a screen row (empty when unknown); rows
    /// are joined with `\n`, and trailing blanks of each row are dropped.
    pub fn text(&self, line_of: impl Fn(u32) -> String) -> String {
        let (start, end) = self.ends();
        let mut out = Vec::new();
        for row in start.row..=end.row {
            let chars: Vec<char> = line_of(row).chars().collect();
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
                        chars.len()
                    };
                    (from, to)
                }
                Shape::Block => {
                    let (c0, c1) = (start.col.min(end.col), start.col.max(end.col));
                    (c0 as usize, c1 as usize + 1)
                }
            };
            let from = from.min(chars.len());
            let to = to.min(chars.len()).max(from);
            let piece: String = chars[from..to].iter().collect();
            out.push(piece.trim_end().to_owned());
        }
        out.join("\n")
    }
}

impl Endpoint {
    fn at(pos: CellPos, line: &str, granularity: Granularity) -> Self {
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
pub fn snap(granularity: Granularity, line: &str, col: u16) -> (u16, u16) {
    match granularity {
        Granularity::Cells => (col, col),
        Granularity::Words => word_span(line, col),
        Granularity::Lines => {
            let last = line.chars().count().saturating_sub(1);
            (0, u16::try_from(last).unwrap_or(u16::MAX))
        }
    }
}

/// The run of one character class around `col`: word characters (letters, digits, `_`),
/// blanks, or anything else. Past the end of the line, the cell itself.
pub fn word_span(line: &str, col: u16) -> (u16, u16) {
    let chars: Vec<char> = line.chars().collect();
    let at = col as usize;
    if at >= chars.len() {
        return (col, col);
    }
    let class = class_of(chars[at]);
    let mut from = at;
    while from > 0 && class_of(chars[from - 1]) == class {
        from -= 1;
    }
    let mut to = at;
    while to + 1 < chars.len() && class_of(chars[to + 1]) == class {
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

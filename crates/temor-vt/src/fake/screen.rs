//! The fake's grid: one `char` per cell, with scrollback as plain lines.

use crate::GridSize;

#[derive(Debug)]
pub(super) struct Screen {
    pub(super) size: GridSize,
    rows: Vec<Vec<char>>,
    scrollback: Vec<(String, Wrap)>,
    /// Lines that ended by wrapping, by row, so text export can join them.
    wrapped: Vec<Wrap>,
    pub(super) col: u16,
    pub(super) row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wrap {
    Hard,
    Soft,
}

impl Screen {
    pub(super) fn new(size: GridSize) -> Self {
        let (cols, rows) = (usize::from(size.cols.0), usize::from(size.rows.0));
        Self {
            size,
            rows: vec![vec![' '; cols]; rows],
            scrollback: Vec::new(),
            wrapped: vec![Wrap::Hard; rows],
            col: 0,
            row: 0,
        }
    }

    pub(super) fn print(&mut self, c: char) {
        if self.col >= self.size.cols.0 {
            self.wrapped[usize::from(self.row)] = Wrap::Soft;
            self.col = 0;
            self.linefeed();
        }
        if let Some(cell) = self
            .rows
            .get_mut(usize::from(self.row))
            .and_then(|r| r.get_mut(usize::from(self.col)))
        {
            *cell = c;
        }
        self.col += 1;
    }

    pub(super) fn linefeed(&mut self) {
        if self.row + 1 < self.size.rows.0 {
            self.row += 1;
            return;
        }
        let top: String = self.rows.remove(0).into_iter().collect();
        let wrap = self.wrapped.remove(0);
        self.scrollback.push((top, wrap));
        self.rows.push(vec![' '; usize::from(self.size.cols.0)]);
        self.wrapped.push(Wrap::Hard);
    }

    pub(super) fn carriage_return(&mut self) {
        self.col = 0;
    }

    pub(super) fn backspace(&mut self) {
        self.col = self.col.saturating_sub(1);
    }

    pub(super) fn tab(&mut self) {
        self.col = ((self.col / 8) + 1) * 8;
        self.col = self.col.min(self.size.cols.0.saturating_sub(1));
    }

    pub(super) fn goto(&mut self, row: u16, col: u16) {
        self.row = row.min(self.size.rows.0.saturating_sub(1));
        self.col = col.min(self.size.cols.0.saturating_sub(1));
    }

    pub(super) fn erase_display(&mut self) {
        let size = self.size;
        let (col, row) = (self.col, self.row);
        *self = Self {
            scrollback: std::mem::take(&mut self.scrollback),
            ..Self::new(size)
        };
        self.goto(row, col);
    }

    /// Resizes without reflow: rows are cut or padded.
    pub(super) fn resize(&mut self, size: GridSize) {
        let (cols, rows) = (usize::from(size.cols.0), usize::from(size.rows.0));
        for r in &mut self.rows {
            r.resize(cols, ' ');
        }
        self.rows.resize(rows, vec![' '; cols]);
        self.wrapped.resize(rows, Wrap::Hard);
        self.size = size;
        self.goto(self.row, self.col);
    }

    /// Scrollback then screen, soft wraps joined, trailing blanks and empty lines dropped.
    pub(super) fn text(&self) -> String {
        let screen = self
            .rows
            .iter()
            .map(|r| r.iter().collect::<String>())
            .zip(self.wrapped.iter().copied());
        let all = self.scrollback.iter().cloned().chain(screen);
        let mut lines: Vec<String> = Vec::new();
        let mut joining = false;
        for (line, wrap) in all {
            match (joining, lines.last_mut()) {
                (true, Some(prev)) => prev.push_str(&line),
                _ => lines.push(line),
            }
            joining = wrap == Wrap::Soft;
        }
        let mut lines: Vec<String> = lines.into_iter().map(|l| l.trim_end().to_owned()).collect();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines.join("\n")
    }
}

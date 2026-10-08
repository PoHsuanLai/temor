//! Shared helpers: a terminal wired to a channel, fixtures, and owned copies of frame cells.

use std::sync::mpsc;

use temor_vt::{
    CellStyle, CellWidth, GridSize, Hyperlink, RenderView, Rgb, RowFilter, SemanticMark, Terminal,
    TerminalConfig, VtEvent,
};

/// A `cols` x `rows` terminal and the receiving end of its events.
pub fn term(cols: u16, rows: u16) -> (Terminal, mpsc::Receiver<VtEvent>) {
    let (tx, rx) = mpsc::channel();
    let term = Terminal::new(TerminalConfig::new(GridSize::new(cols, rows)), Box::new(tx))
        .expect("terminal");
    (term, rx)
}

/// The bytes of `tests/fixtures/<name>`.
pub fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every event sent so far.
pub fn drain(rx: &mpsc::Receiver<VtEvent>) -> Vec<VtEvent> {
    rx.try_iter().collect()
}

/// An owned copy of one frame cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub width: CellWidth,
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub style: CellStyle,
    pub hyperlink: Hyperlink,
    pub semantic: SemanticMark,
}

/// The cells of viewport row `row`, read through a fresh render view.
pub fn row_cells(term: &mut Terminal, row: u16) -> Vec<Cell> {
    let mut view = RenderView::new().expect("view");
    let mut frame = view.begin(term).expect("begin").end().expect("end");
    let mut cells = Vec::new();
    frame
        .rows(RowFilter::All, |r| {
            if r.row() == row {
                r.cells(|c| {
                    cells.push(Cell {
                        text: c.text.to_owned(),
                        width: c.width,
                        fg: c.fg,
                        bg: c.bg,
                        style: c.style,
                        hyperlink: c.hyperlink,
                        semantic: c.semantic,
                    })
                })
                .expect("cells");
            }
        })
        .expect("rows");
    cells
}

/// The texts of viewport row `row`, one per cell.
pub fn row_texts(term: &mut Terminal, row: u16) -> Vec<String> {
    row_cells(term, row).into_iter().map(|c| c.text).collect()
}

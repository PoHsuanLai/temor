//! Whether a prompt leads with a blank line above its header (temor's prompt does): a block that
//! begins on that line begins across its middle. `leads_blank` is pure; `lead_at` reads the
//! terminal.

use temor_core::session::frame::Lead;
use temor_pane::Term;
use temor_pane::text::RowText;
use temor_vt::{AnchorId, PointSpace};

/// Whether a prompt's first row is a blank line: nothing but spaces, and a row below it that the
/// prompt goes on to (its header).
pub(super) fn leads_blank(first: &RowText, input_row: Option<i64>, first_row: i64) -> Lead {
    let blank = first.cells.iter().all(|cell| cell.text.trim().is_empty());
    match blank && input_row.is_some_and(|input| input > first_row) {
        true => Lead::Blank,
        false => Lead::Header,
    }
}

/// What a prompt's first row is in the terminal, from its prompt and input marks; a header when
/// the marks or the row can't be read.
pub(super) fn lead_at(term: &Term, prompt: AnchorId, input: Option<AnchorId>) -> Lead {
    let row_of = |id| term.anchor_point(id, PointSpace::Screen).map(|p| p.row);
    let Some(first) = row_of(prompt) else {
        return Lead::Header;
    };
    let input = input.and_then(row_of).map(i64::from);
    term.row_text(first).map_or(Lead::Header, |text| {
        leads_blank(&text, input, i64::from(first))
    })
}

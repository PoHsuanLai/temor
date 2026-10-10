//! The word under a click, and the paths it might be.

use temor_pane::text::{RowText, Span};

/// The longest token worth looking at: a path longer than this is not a path a person clicked.
const LONGEST: usize = 4096;

/// What a path is wrapped in when it is printed: quotes, brackets, a comma after it.
fn is_wrapping(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '<' | '>' | '{' | '}' | ',' | ';'
    )
}

/// The run of non-blank cells around column `col` of `row`, as text; `None` on a blank cell, past
/// the end of the row, or for a run too long to be a path. A wide character's spacer belongs to
/// the character before it.
pub fn token_at(row: &RowText, col: u16) -> Option<String> {
    let cells = &row.cells;
    let blank = |index: usize| -> bool {
        let mut at = index;
        while at > 0 && cells.get(at).is_some_and(|c| c.span == Span::Spacer) {
            at -= 1;
        }
        cells.get(at).is_none_or(|c| c.text.trim().is_empty())
    };
    let at = usize::from(col);
    if at >= cells.len() || blank(at) {
        return None;
    }
    let mut from = at;
    while from > 0 && !blank(from - 1) {
        from -= 1;
    }
    let mut to = at;
    while to + 1 < cells.len() && !blank(to + 1) {
        to += 1;
    }
    let text: String = cells[from..=to].iter().map(|c| c.text.as_str()).collect();
    (text.len() <= LONGEST).then_some(text)
}

/// The paths `token` might name, most likely first: the token without what wraps it, then without
/// a trailing colon, then without `:line` and `:line:column` (a compiler's or `grep -n`'s), then
/// what comes before the first colon (`rg`'s `path:line:text`). Empty ones and repeats are left
/// out. Nothing here looks at the disk.
pub fn candidates(token: &str) -> Vec<String> {
    let base = token.trim_matches(is_wrapping);
    let mut out: Vec<String> = Vec::new();
    let mut push = |text: &str| {
        if !text.is_empty() && !out.iter().any(|seen| seen == text) {
            out.push(text.to_owned());
        }
    };
    push(base);
    let mut peeled = base.trim_end_matches(':');
    push(peeled);
    while let Some((head, tail)) = peeled.rsplit_once(':') {
        if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        peeled = head;
        push(peeled);
    }
    if let Some((head, _)) = base.split_once(':') {
        push(head);
    }
    out
}

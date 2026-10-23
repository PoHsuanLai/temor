//! A finished or running command as a card (PLAN M9): over the rows of its header line and
//! command, an opaque surface that says the command large, one line of what it printed under
//! it, and nothing else unless something went wrong. Status is colour for the exception only:
//! a failed card is tinted and says its exit status in words; a running one is amber. How long
//! it took is the bar on its bottom edge, against the longest card in view; how much it printed
//! is the glyph at the right; older cards are fainter.
//!
//! Cards sit beside the grid, so a press never reaches the terminal, and they are not drawn on a
//! full-screen program's screen. What they hide is still in the terminal: copy and find see it.

use dioxus::prelude::*;
use ds::base::text::clip::clip_chars;
use temor_core::block::{NoteTone, Outcome, age_strength, bar_share, format_elapsed, glyph_bars};
use temor_core::vocab::ids::BlockId;
use temor_render::Palette;

use super::card_colours::{CardFill, card_ground};
use crate::session::{Frame, Session};
use temor_core::session::frame::Lead;

/// The room kept above and below a card, in logical pixels. None: the grid text under the card
/// would show in the gap.
const CARD_GAP: f32 = 0.0;

/// How a card stands, which sets its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardState {
    Done,
    Failed,
    Running,
}

impl CardState {
    fn slug(self) -> &'static str {
        match self {
            CardState::Done => "done",
            CardState::Failed => "failed",
            CardState::Running => "running",
        }
    }
}

/// One card, placed by the window from the session.
#[derive(Debug, Clone, PartialEq)]
pub struct CardView {
    pub id: BlockId,
    /// The card's top and height in logical pixels.
    pub top: f32,
    pub height: f32,
    /// One grid row's height, in logical pixels.
    pub row: f32,
    /// The pane's padding in logical pixels.
    pub inset: f32,
    /// The command's first line.
    pub title: String,
    /// The line of output under it, if the command printed any.
    pub note: Option<(String, NoteTone)>,
    pub state: CardState,
    /// "exit 1" for a failure.
    pub status: Option<String>,
    /// The tooltip: how it ended and how long it ran.
    pub tip: String,
    /// The part of the card's width the duration bar takes.
    pub bar: f32,
    /// The bars of the output glyph, 0 when there was no output.
    pub bars: u8,
    /// How strongly it draws, 0 to 1.
    pub strength: f32,
    /// The card's ground as CSS when it has a fill; the quire surface otherwise.
    pub ground: Option<String>,
    /// The folder, on the row above the card, when it differs from the card before's.
    pub folder: Option<String>,
}

/// The state and words for a block's outcome.
fn standing(outcome: Outcome) -> (CardState, Option<String>) {
    match outcome {
        Outcome::Running => (CardState::Running, None),
        Outcome::Failed(code) => (CardState::Failed, Some(format!("exit {}", code.0))),
        _ => (CardState::Done, None),
    }
}

/// The cards over the blocks on screen: those whose header and command rows are in view.
pub fn cards_of(session: &Session, frame: &Frame, palette: &Palette) -> Vec<CardView> {
    let Some(gutter) = session.gutter_geometry() else {
        return Vec::new();
    };
    let rows = session.rows();
    let columns = session
        .fit()
        .map_or(80, |fit| usize::from(fit.grid.cols.0))
        .saturating_sub(12);
    let end = frame.cursor_row().unwrap_or(i64::from(rows) - 1);
    let marks = frame.marks();
    let spans: Vec<_> = frame
        .card_spans(end)
        .into_iter()
        .filter(|span| span.first >= 0 && span.last < i64::from(rows))
        .collect();
    let longest = marks
        .iter()
        .filter_map(|mark| mark.took_ms)
        .max()
        .unwrap_or(0);
    let count = spans.len();
    let mut last_folder: Option<String> = None;
    spans
        .into_iter()
        .enumerate()
        .filter_map(|(n, span)| {
            let mark = marks.iter().find(|mark| mark.id == span.id)?;
            let title = session
                .block_command(span.id)
                .and_then(|command| command.lines().next())
                .map(|line| clip_chars(line.trim(), columns))
                .filter(|line| !line.is_empty())?;
            let (state, status) = standing(mark.outcome);
            let note = session.block_note(span.id);
            let fill = match (session.block_picked(span.id), state) {
                (true, _) => CardFill::Picked,
                (false, CardState::Failed) => CardFill::Failed,
                _ => CardFill::None,
            };
            // Under the blank line temor's prompt leaves above the header.
            let lead = i64::from(span.lead == Lead::Blank && span.first < span.last);
            let (top_row, last_row) = (span.first + lead, span.last);
            let first = top_row as u32;
            // A command with no output yet runs to the cursor; the card is the header and the
            // command's own lines.
            let own = i64::try_from(
                session
                    .block_command(span.id)
                    .map_or(1, |c| c.lines().count().max(1)),
            )
            .unwrap_or(1);
            let rows = (last_row - top_row + 1).min(1 + own);
            // A new folder is named on the blank row above the card, when there is one on screen.
            let folder = session
                .header_facts(span.id)
                .map(|kept| kept.folder.clone());
            let named = folder
                .clone()
                .filter(|folder| !folder.is_empty() && last_folder.as_ref() != Some(folder))
                .filter(|_| lead == 1 && span.first >= 0);
            if folder.is_some() {
                last_folder = folder;
            }
            let tip = [
                status.clone(),
                mark.took_ms.map(format_elapsed),
                note.map(|note| format!("{} lines", note.lines)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" \u{b7} ");
            let hovered = session.hovered_block() == Some(span.id);
            Some(CardView {
                id: span.id,
                top: gutter.row_top(first) + CARD_GAP,
                height: rows as f32 * gutter.cell_h - 2.0 * CARD_GAP,
                row: gutter.cell_h,
                inset: gutter.side,
                title,
                note: note.map(|note| (clip_chars(&note.text, columns), note.tone)),
                state,
                status,
                tip,
                bar: bar_share(mark.took_ms.unwrap_or(0), longest),
                bars: note.map_or(0, |note| glyph_bars(note.lines)),
                strength: match hovered || session.block_picked(span.id) {
                    true => 1.0,
                    false => age_strength(count - 1 - n),
                },
                ground: card_ground(palette, fill),
                folder: named,
            })
        })
        .collect()
}

/// The cards over the blocks on screen.
#[component]
#[allow(non_snake_case)]
pub fn BlockCards(cards: Vec<CardView>) -> Element {
    rsx! {
        for card in cards.iter().filter(|card| card.folder.is_some()) {
            if let Some(folder) = card.folder.clone() {
                div {
                    key: "f{card.id.0}",
                    class: "temor-card-folder ds-mono",
                    style: "top:{card.top - card.row}px; height:{card.row}px; left:{card.inset}px; right:{card.inset}px",
                    "{folder}"
                }
            }
        }
        for card in cards {
            div {
                key: "{card.id.0}",
                class: "temor-card",
                "data-state": card.state.slug(),
                title: "{card.tip}",
                style: card_style(&card),
                div { class: "temor-card-body", style: "opacity:{card.strength}",
                div { class: "temor-card-head",
                    span { class: "temor-card-title ds-mono", "{card.title}" }
                    if let Some(status) = card.status.clone() {
                        span { class: "temor-card-status ds-mono", "{status}" }
                    }
                    if card.bars > 0 {
                        span { class: "temor-card-glyph",
                            for n in 0..card.bars {
                                i { key: "{n}" }
                            }
                        }
                    }
                }
                if let Some((text, tone)) = card.note.clone() {
                    div {
                        class: "temor-card-note ds-mono",
                        "data-tone": if tone == NoteTone::Trouble { "trouble" } else { "plain" },
                        "{text}"
                    }
                } else if card.state == CardState::Running {
                    div { class: "temor-card-note ds-mono", "running" }
                }
                }
                if card.state != CardState::Running {
                    span {
                        class: "temor-card-bar",
                        style: "width:{card.bar * 100.0}%; opacity:{card.strength}",
                    }
                }
            }
        }
    }
}

fn card_style(card: &CardView) -> String {
    let ground = card
        .ground
        .as_ref()
        .map(|ground| format!("; background:{ground}"))
        .unwrap_or_default();
    format!(
        "top:{}px; height:{}px; left:{}px; right:{}px{ground}",
        card.top, card.height, card.inset, card.inset
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_core::vocab::ids::ExitCode;

    #[test]
    fn only_a_failure_says_its_status() {
        assert_eq!(standing(Outcome::Succeeded), (CardState::Done, None));
        assert_eq!(standing(Outcome::Running), (CardState::Running, None));
        assert_eq!(
            standing(Outcome::Failed(ExitCode(101))),
            (CardState::Failed, Some("exit 101".to_owned()))
        );
        assert_eq!(standing(Outcome::Stopped(ExitCode(130))).0, CardState::Done);
    }
}

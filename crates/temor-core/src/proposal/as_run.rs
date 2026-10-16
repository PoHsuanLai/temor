//! Telling an edited proposal from a replaced one (§8.3).

use super::step::AsRun;

/// How `ran` relates to `proposed`. Exactly equal is as proposed. Otherwise it is an edit when
/// the common prefix and the common suffix (counted in characters, not overlapping, the braille
/// blank read as a space) together cover at least half of the proposal; anything less means the
/// person wiped the proposal and typed their own command.
pub(super) fn classify(proposed: &str, ran: &str) -> AsRun {
    let plain = |text: &str| -> Vec<char> {
        text.chars()
            .map(|c| if c == '\u{2800}' { ' ' } else { c })
            .collect()
    };
    let (a, b) = (plain(proposed), plain(ran));
    if a == b {
        return AsRun::AsProposed;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let room = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();
    if (prefix + suffix) * 2 >= a.len() {
        AsRun::Edited
    } else {
        AsRun::Replaced
    }
}

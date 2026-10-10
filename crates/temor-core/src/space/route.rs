//! Routes send new sessions to a Space (PLAN §7): `ssh prod-*` opens in Ops. Pure matching over
//! the patterns the Spaces hold.
//!
//! Two kinds of pattern, told apart by their first character:
//! - A **folder** pattern starts with `/` or `~`: it matches a folder and everything under it
//!   (`~/work` matches `~/work` and `~/work/api`, not `~/workshop`). Written paths are compared
//!   as text; the edge expands `~` the same way on both sides.
//! - Anything else is a **command** pattern: a glob over the whole command line, where `*`
//!   matches any run of characters (spaces too) and `?` matches one. Runs of blanks in the
//!   command count as one space and the ends are trimmed. Case matters.
//!
//! When several routes match, the most specific wins: the most literal characters in a command
//! pattern, the longest folder. A tie goes to the earlier Space, then the earlier route.

use super::project::Project;
use ds_style::space::list::{Space, SpaceId};

use crate::vocab::text::RoutePattern;

/// One route with the Space it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceRoute<'a> {
    /// Where a match opens.
    pub space: SpaceId,
    /// What it matches.
    pub pattern: &'a RoutePattern,
}

/// Every route of every Space, in the Spaces' order.
pub fn routes_of(spaces: &[Space<Project>]) -> Vec<SpaceRoute<'_>> {
    spaces
        .iter()
        .flat_map(|space| {
            space.payload.routes.iter().map(|route| SpaceRoute {
                space: space.id,
                pattern: &route.pattern,
            })
        })
        .collect()
}

fn is_folder_pattern(pattern: &str) -> bool {
    pattern.starts_with(['/', '~'])
}

/// The Space a new session running `command` opens in, if a command pattern matches.
pub fn route(command: &str, routes: &[SpaceRoute<'_>]) -> Option<SpaceId> {
    let command = command.split_whitespace().collect::<Vec<_>>().join(" ");
    best(routes, |pattern| {
        let pattern = pattern.trim();
        let usable = !pattern.is_empty() && !is_folder_pattern(pattern);
        (usable && glob(pattern, &command)).then(|| literal_chars(pattern))
    })
}

/// The Space a new session started in `folder` opens in, if a folder pattern matches.
pub fn route_folder(folder: &str, routes: &[SpaceRoute<'_>]) -> Option<SpaceId> {
    let folder = folder.trim_end_matches('/');
    best(routes, |pattern| {
        let pattern = pattern.trim().trim_end_matches('/');
        let under = folder
            .strip_prefix(pattern)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
        (is_folder_pattern(pattern) && under).then(|| pattern.chars().count())
    })
}

/// The route with the highest score, the earliest on a tie.
fn best(routes: &[SpaceRoute<'_>], score: impl Fn(&str) -> Option<usize>) -> Option<SpaceId> {
    let mut best: Option<(usize, SpaceId)> = None;
    for route in routes {
        let Some(points) = score(route.pattern.as_str()) else {
            continue;
        };
        if best.is_none_or(|(top, _)| points > top) {
            best = Some((points, route.space));
        }
    }
    best.map(|(_, space)| space)
}

fn literal_chars(pattern: &str) -> usize {
    pattern.chars().filter(|c| !matches!(c, '*' | '?')).count()
}

/// Whether `pattern` matches all of `text`: `*` any run (also empty), `?` one character.
fn glob(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Where the last `*` was, and the text position it is currently stretched to.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(c) if *c == '?' || *c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((star_at, stretched)) => {
                    star = Some((star_at, stretched + 1));
                    p = star_at + 1;
                    t = stretched + 1;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

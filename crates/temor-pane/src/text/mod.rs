//! The pane's text: rows of cells, selections over them, search, and the rules for links and
//! working directories. Pure: the caller supplies the rows.

mod cwd;
mod link;
mod row;
mod search;
mod selection;
#[cfg(test)]
mod selection_tests;

pub use cwd::cwd_from_url;
pub use link::{LinkRefusal, LinkTarget, LinkVerdict, check_link};
pub use row::{RowCell, RowText, Span, Wrap};
pub use search::{Finder, Match, find_all, may_match};
pub use selection::{CellPos, Granularity, Selection, Shape};

//! The blocks of a pane as placed in its viewport: plain data read from the terminal (`read`),
//! the questions the views ask of it (`view`), and the row arithmetic under them (`place`,
//! `card`).

mod card;
mod place;
mod read;
mod view;

pub use card::{CardBlock, Lead, Mark, Pick, Stage};
pub use place::{HEADER_MIN_ROWS, Span, ViewRows, command_rows, heads_viewport, tools_row};
pub use read::{BlockRead, End, LiveRead, Read, Spot};
pub use view::{BlockMark, CardSpan, Frame, HeaderLine, PromptHeader, StickyHeader, ToolsSpot};

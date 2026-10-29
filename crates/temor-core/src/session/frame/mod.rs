//! The blocks of a pane as placed in its viewport: plain data read from the terminal (`read`),
//! the questions the list asks of it (`view`), and the row arithmetic under them (`place`).

mod place;
mod read;
mod view;

pub use place::command_rows;
pub use read::{BlockRead, End, Lead, LiveRead, Read, Spot};
pub use view::{BlockMark, Frame};

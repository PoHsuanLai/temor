//! The downloads shelf as a pure model (§9): what arrived, how far it got, where it is saved, and
//! what the shelf shows. The edges create files, open them and read the folder; this module only
//! decides.

mod drop;
mod model;
mod naming;
mod step;
#[cfg(test)]
mod tests;
mod view;

pub use drop::{DropAction, Dropped, PaneSite, Quoting, ReceiveSupport, drop_action, quote};
pub use model::{Clear, FailWhy, Incoming, ItemState, Seen, Shelf, ShelfItem, Source};
pub use naming::{candidate_name, safe_name};
pub use step::{ShelfIn, ShelfOut, shelf_step};
pub use view::{Row, RowAction, RowState, ShelfView, shelf_view};

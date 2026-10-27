//! The plugins' work (PLAN D14, P11): temor's side of `temor-plugins`, which finds and runs
//! them. Everything a plugin returns arrives on a channel from a worker thread, never on the
//! window's; a plugin that fails shows nothing and is logged.
//!
//! - `host`: the catalog, the launchers and the channel results come back on;
//! - `facts`: `row.facts` as chips' words and the sidebar row's summary;
//! - `offer`: `open.provider` rows for the bar;
//! - `effects`: carrying out a chosen row (copy, open, nothing);
//! - `adapter`: `agent.adapter` events as an agent's status.
//!
//! What the window keeps of them and draws (lens pictures, the hub's state) is the window's.

pub mod adapter;
pub mod effects;
pub mod facts;
pub mod host;
pub mod offer;

// What the window keeps and draws of the plugins: their catalog and data folders, a lens's
// picture and the rows they offer.
pub use temor_plugins::{
    DataDirs, Drawing, FOLDER_TABLE, Generations, Plugin, Routed, effect_of, lens_block, message,
};

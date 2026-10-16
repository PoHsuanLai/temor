//! Plugins in the window (PLAN D14, P11): temor's side of `temor-plugins`, which finds and runs
//! them. Everything a plugin returns arrives on a channel from a worker thread, never on the
//! window's; a plugin that fails shows nothing and is logged.
//!
//! - `host`: the catalog, the launchers and the channel results come back on;
//! - `state`: what the hub keeps (facts, offered rows, lens pictures, running adapters);
//! - `facts`: `row.facts` as chips' words and the sidebar row's summary;
//! - `offer`: `open.provider` rows for the bar;
//! - `effects`: carrying out a chosen row (copy, open, nothing);
//! - `lens`: `block.lens` pictures;
//! - `adapter`: `agent.adapter` events as an agent's status.
//!
//! Two files are mounted from elsewhere as children, so they can reach private state:
//! `provider_rows.rs` from `bar/adapter.rs` (the bar's model) and `glue.rs` from `hub/mod.rs`
//! (the hub's fields).

pub mod adapter;
pub mod effects;
pub mod facts;
pub mod hands;
pub mod host;
pub mod lens;
pub mod offer;
pub mod state;

pub use host::{Done, PluginHost};
pub use state::PluginHub;

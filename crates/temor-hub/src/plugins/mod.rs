//! Plugins in the window (PLAN D14, P11): temor's side of `temor-plugins`, which finds and runs
//! them. Everything a plugin returns arrives on a channel from a worker thread, never on the
//! window's; a plugin that fails shows nothing and is logged.
//!
//! - `state`: what the hub keeps (facts, offered rows, lens pictures, running adapters);
//! - `lens`: `block.lens` pictures;
//! - `hands`: what carrying out a chosen row does in the window.
//!
//! The work itself (the catalog and launchers, facts, offers, effects, adapters) is the
//! client's (`temor_client::plugins`).
//!
//! Two files are mounted from elsewhere as children, so they can reach private state:
//! `provider_rows.rs` from `bar/adapter.rs` (the bar's model) and `glue.rs` from `hub/mod.rs`
//! (the hub's fields).

pub mod hands;
pub mod lens;
pub mod state;

pub use temor_client::plugins::{adapter, effects, facts, host, offer};

pub use host::{Done, PluginHost};
pub use state::PluginHub;

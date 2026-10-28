//! Tier A hosts, below the window (§9b, P13): what the bootstrap may do on a host (asked once,
//! remembered per machine), the shared connection that panes join, editing a host's file
//! locally and writing it back, and port forwards on the shared connection. Pure: the app feeds
//! these machines facts and carries out what they return.

pub mod consent;
pub mod edit;
pub mod forward;
pub mod master;
pub mod store;
pub mod target;

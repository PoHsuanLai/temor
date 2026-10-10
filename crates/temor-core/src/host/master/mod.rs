//! One host's shared connection, as panes ask for it (§9b "Opening", "Split and new tab inherit
//! the host"). The first pane makes the master; panes asked for while it connects wait, so a
//! password or passphrase is asked once, then each joins at once. A master that survived from
//! an earlier run is found by a probe and reused.

mod step;
#[cfg(test)]
mod tests;

pub use step::{Master, MasterIn, MasterOut, OpenAs, master_step};

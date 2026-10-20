//! The Linux desktop: the session bus, docket's agent layer and porter's account service.

mod args;
mod bus;
mod checkpoints;
mod context;
mod dock;
mod instance;
mod intents;
mod links;
mod perform;
mod secrets;
mod tailnet;
#[cfg(test)]
mod tests;
mod watching;

pub use checkpoints::SystemCheckpoints;
pub use dock::SystemDock;
pub use instance::SystemInstance;
pub use intents::SystemIntents;
pub use links::SystemLinks;
pub use secrets::SystemSecrets;
pub use tailnet::SystemTailnet;

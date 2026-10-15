//! The Linux desktop: the session bus, docket's agent layer and porter's account service.

mod args;
mod bus;
mod checkpoints;
mod context;
mod dock;
mod instance;
mod intents;
mod perform;
mod secrets;
#[cfg(test)]
mod secrets_tests;
mod tailnet;
#[cfg(test)]
mod tests;

pub use checkpoints::SystemCheckpoints;
pub use dock::SystemDock;
pub use instance::SystemInstance;
pub use intents::SystemIntents;
pub use secrets::SystemSecrets;
pub use tailnet::SystemTailnet;

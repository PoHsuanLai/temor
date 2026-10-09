//! The command-line subcommands `temor save`, `temor get` and `temor view`, for use inside a temor
//! pane, on this computer or over SSH. They talk to the window with the file transfer protocol
//! over the terminal, so they work through SSH unchanged.

mod args;
mod dispatch;
mod error;
mod exec;
mod opener;
mod platform;
#[cfg(test)]
mod tests;
mod text;
mod view;

pub use dispatch::dispatch;

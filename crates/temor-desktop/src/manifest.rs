//! The names temor owns on the bus and the actions it declares. The declarations themselves are
//! the file `dist/intents/org.quire.Temor.toml`, embedded here so the file the installer ships
//! and the one the window answers for are one file.
//!
//! Action names start with `temor`, the app name's last element, which the agent layer requires
//! (PLAN §8.4 writes them as `terminal.*`; the validator refuses any other prefix).

/// The name the agent layer calls (the manifest's app).
pub const APP_BUS_NAME: &str = "org.quire.Temor";
/// The name the first launch owns; a later launch calls it (single instance).
pub const INSTANCE_BUS_NAME: &str = "org.quire.Temor1";

/// The shipped actions file.
pub const MANIFEST: &str = include_str!("../../../dist/intents/org.quire.Temor.toml");

/// List the panes.
pub const PANE_LIST: &str = "temor.pane.list";
/// A pane's working folder.
pub const PANE_CWD: &str = "temor.pane.cwd";
/// Read a pane's text.
pub const PANE_READ: &str = "temor.pane.read";
/// Open a pane.
pub const PANE_OPEN: &str = "temor.pane.open";
/// List a pane's commands.
pub const COMMAND_LIST: &str = "temor.command.list";
/// Read one command's output.
pub const COMMAND_READ: &str = "temor.command.read";
/// Run a command in a sandbox.
pub const COMMAND_RUN_SANDBOXED: &str = "temor.command.run_sandboxed";
/// Type a command into a pane as the person.
pub const COMMAND_RUN_IN_PANE: &str = "temor.command.run_in_pane";

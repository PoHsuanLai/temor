//! temor's plugins (PLAN D14, P11), on bayonet.
//!
//! A plugin is a program a person installs; temor runs it and never links it. This crate holds
//! temor's side of the arrangement:
//!
//! - [`capability`]: the four capabilities (`block.lens`, `open.provider`, `agent.adapter`,
//!   `row.facts`) and how a manifest's `[[provides]]` entry reads for each;
//! - [`message`]: what the two sides say to each other;
//! - [`catalog`]: finding the manifests in `<data dir>/temor/plugins` and choosing the plugins that
//!   serve a request;
//! - [`tidy`], [`effect`], [`drawing`]: what a plugin says is cleaned and bounded before it is shown,
//!   a chosen row maps onto the bar's own actions (copy, open, nothing), a picture is checked;
//! - [`Launcher`]: starting a plugin for one request (Unix; elsewhere every request answers
//!   [`PluginError::Unsupported`]).
//!
//! Nothing here lets an agent act: a plugin that lets someone do something an agent might ask for
//! is an action in the app's intents manifest, gated like any other (plugins.md §4).

pub mod cancel;
pub mod capability;
pub mod catalog;
pub mod drawing;
pub mod effect;
pub mod error;
pub mod message;
mod platform;
pub mod tidy;
pub mod trigger;
pub mod waits;

pub use cancel::Generations;
pub use capability::{Cap, FOLDER_TABLE, Fault, Provision};
pub use catalog::{APP, Catalog, DataDirs, PROTOCOL, Plugin, Routed};
pub use drawing::{BadDrawing, Drawing, MOST_DRAWING_BYTES, MOST_OUTPUT_BYTES, lens_block};
pub use effect::{Effect, Refused, effect_of};
pub use error::{CancelKind, PluginError};
pub use platform::Launcher;
pub use trigger::Trigger;
pub use waits::Waits;

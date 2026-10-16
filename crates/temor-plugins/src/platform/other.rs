//! Systems without bayonet's run half: no plugin runs.
//!
//! TODO(platform:windows): the same methods as the Unix half, over job objects.

use crate::cancel::Cancel;
use crate::catalog::Plugin;
use crate::drawing::Drawing;
use crate::error::PluginError;
use crate::message::{Activation, Batch, Choice, Fact, Item, LensBlock, Phase};
use crate::waits::Waits;

/// A launcher that cannot run anything.
#[derive(Debug, Clone)]
pub struct Launcher;

impl Launcher {
    /// A launcher; the waits and the log are not used.
    pub fn new(_waits: Waits, _log: impl Fn(&str) + Send + Sync + 'static) -> Launcher {
        Launcher
    }

    /// `row.facts`.
    pub fn facts(&self, _: &Plugin, _table: &str, _key: &str) -> Result<Vec<Fact>, PluginError> {
        Err(PluginError::Unsupported)
    }

    /// `open.provider`: the query.
    pub fn query(
        &self,
        _: &Plugin,
        _text: &str,
        _generation: u64,
        _cancel: &dyn Fn() -> Cancel,
        _on_batch: &mut dyn FnMut(Batch),
    ) -> Result<(), PluginError> {
        Err(PluginError::Unsupported)
    }

    /// `open.provider`: a chosen row.
    pub fn activate(
        &self,
        _: &Plugin,
        _item: &Item,
        _choice: &Choice,
    ) -> Result<Activation, PluginError> {
        Err(PluginError::Unsupported)
    }

    /// `block.lens`.
    pub fn lens(&self, _: &Plugin, _block: &LensBlock) -> Result<Drawing, PluginError> {
        Err(PluginError::Unsupported)
    }

    /// `agent.adapter`.
    pub fn adapt(
        &self,
        _: &Plugin,
        _agent: &str,
        _folder: &str,
        _cancel: &dyn Fn() -> Cancel,
        _on_event: &mut dyn FnMut(Phase, String),
    ) -> Result<(), PluginError> {
        Err(PluginError::Unsupported)
    }
}

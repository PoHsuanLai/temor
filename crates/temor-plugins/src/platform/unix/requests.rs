//! One method for each thing temor asks of a plugin. What comes back is cleaned (`tidy`) before
//! it leaves this module.

use std::ops::ControlFlow;

use super::launcher::{Launcher, said, unexpected};
use crate::capability::Cap;
use crate::catalog::Plugin;
use crate::drawing::{Drawing, MOST_DRAWING_BYTES};
use crate::error::PluginError;
use crate::message::{Activation, Batch, Choice, Fact, Item, LensBlock, Message, Phase, Request};
use crate::tidy::{tidy_batch, tidy_facts};

/// A plugin that says it failed after temor asked it to stop was stopped, not broken.
fn failure(plugin: &Plugin, cancel: &dyn Fn() -> bool, message: String) -> PluginError {
    match cancel() {
        true => PluginError::Cancelled {
            plugin: plugin.id().to_owned(),
        },
        false => said(plugin, message),
    }
}

impl Launcher {
    /// `row.facts`: what `plugin` knows about the row `key` of `table`. Blocking.
    pub fn facts(&self, plugin: &Plugin, table: &str, key: &str) -> Result<Vec<Fact>, PluginError> {
        let request = Request::Facts {
            table: table.to_owned(),
            key: key.to_owned(),
        };
        let mut session = self.open(plugin, Cap::RowFacts, &request, 0)?;
        match session.reply()?.message {
            Message::Facts(facts) => Ok(tidy_facts(facts)),
            Message::Failed { message } => Err(said(plugin, message)),
            other => Err(unexpected(plugin, &other)),
        }
    }

    /// `open.provider`: asks `plugin` for rows for `text`, the keystroke `generation`. Each batch
    /// that answers that keystroke goes to `on_batch`. `cancel` is looked at between messages;
    /// when it says stop, the plugin is asked to stop and the result is
    /// [`PluginError::Cancelled`]. Blocking.
    pub fn query(
        &self,
        plugin: &Plugin,
        text: &str,
        generation: u64,
        cancel: &dyn Fn() -> bool,
        on_batch: &mut dyn FnMut(Batch),
    ) -> Result<(), PluginError> {
        let request = Request::Query {
            text: text.to_owned(),
            generation,
        };
        let mut session = self.open(plugin, Cap::OpenProvider, &request, 0)?;
        session.stream(
            |_| cancel(),
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, PluginError> {
                match frame.message {
                    Message::Batch(batch) => {
                        if let Some(batch) = tidy_batch(batch, generation) {
                            on_batch(batch);
                        }
                        Ok(ControlFlow::Continue(()))
                    }
                    Message::Complete => Ok(ControlFlow::Break(())),
                    Message::Failed { message } => Err(failure(plugin, cancel, message)),
                    other => Err(unexpected(plugin, &other)),
                }
            },
        )
    }

    /// `open.provider`: what choosing `choice` on `item` does. Blocking.
    pub fn activate(
        &self,
        plugin: &Plugin,
        item: &Item,
        choice: &Choice,
    ) -> Result<Activation, PluginError> {
        let request = Request::Activate {
            item: item.clone(),
            choice: choice.clone(),
        };
        let mut session = self.open(plugin, Cap::OpenProvider, &request, 0)?;
        match session.reply()?.message {
            Message::Activation(activation) => Ok(activation),
            Message::Failed { message } => Err(said(plugin, message)),
            other => Err(unexpected(plugin, &other)),
        }
    }

    /// `block.lens`: the picture `plugin` draws for `block`. Blocking.
    pub fn lens(&self, plugin: &Plugin, block: &LensBlock) -> Result<Drawing, PluginError> {
        let request = Request::Lens(block.clone());
        let mut session = self.open(plugin, Cap::BlockLens, &request, MOST_DRAWING_BYTES)?;
        let frame = session.reply()?;
        match frame.message {
            Message::Drawn { width, height } => {
                Drawing::new(width, height, frame.payload).map_err(|bad| PluginError::Broke {
                    plugin: plugin.id().to_owned(),
                    reason: bad.to_string(),
                })
            }
            Message::Failed { message } => Err(said(plugin, message)),
            other => Err(unexpected(plugin, &other)),
        }
    }

    /// `agent.adapter`: runs `plugin` for the agent in `folder` and hands each state it reports to
    /// `on_event`, until it finishes, fails or `cancel` says stop. The host side only: what an
    /// adapter reports is shown, and nothing it says can make temor do anything. Blocking.
    pub fn adapt(
        &self,
        plugin: &Plugin,
        agent: &str,
        folder: &str,
        cancel: &dyn Fn() -> bool,
        on_event: &mut dyn FnMut(Phase, String),
    ) -> Result<(), PluginError> {
        let request = Request::Adapt {
            agent: agent.to_owned(),
            folder: folder.to_owned(),
        };
        let mut session = self.open(plugin, Cap::AgentAdapter, &request, 0)?;
        session.stream(
            |_| cancel(),
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, PluginError> {
                match frame.message {
                    Message::Event { phase, text } => {
                        on_event(phase, text);
                        Ok(ControlFlow::Continue(()))
                    }
                    Message::Finished => Ok(ControlFlow::Break(())),
                    Message::Failed { message } => Err(failure(plugin, cancel, message)),
                    other => Err(unexpected(plugin, &other)),
                }
            },
        )
    }
}

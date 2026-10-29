//! Lenses and adapters in the hub: the other half of the plugins' glue, a child of it.

use std::time::{SystemTime, UNIX_EPOCH};

use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_client::plugins::message::LensBlock;
use temor_client::plugins::{Generations, Plugin, lens_block};
use temor_panes::pane::PaneEvent;

use super::{Desk, Hub, HubRef};
use crate::plugins::adapter::{Follow, follow, status_line};
use crate::plugins::host::{Done, log_trouble};
use crate::plugins::lens::{LensImage, LensOffer, LensPicture};
use crate::plugins::state::Running;

/// Milliseconds since the epoch: the start of an adapter run's sequence numbers, so a run after
/// a restart is never taken for an older report.
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

impl Hub {
    /// The lenses that can draw `block` of `pane`: the plugins whose trigger starts its command.
    pub fn block_lenses(&self, pane: PaneId, block: BlockId) -> Vec<LensOffer> {
        let command = self.live(pane, |s| s.block_command(block)).flatten();
        let Some(command) = command else {
            return Vec::new();
        };
        let lenses = self.plugins.host.catalog.lenses(&command);
        lenses.iter().map(LensOffer::of).collect()
    }

    /// Asks the lens `lens` to draw `block` of `pane`, in `width` pixels. The picture arrives
    /// later; a block that has not finished has nothing to draw.
    pub fn lens_run(&mut self, pane: PaneId, block: BlockId, lens: &str, width: u32) {
        let text = self
            .live(pane, |s| {
                Some((s.block_command(block)?, s.block_output(block)?))
            })
            .flatten();
        let Some((command, output)) = text else {
            return;
        };
        let lenses = self.plugins.host.catalog.lenses(&command);
        let Some(plugin) = lenses.into_iter().find(|p| p.id() == lens) else {
            return;
        };
        let folder = self.pane_folder(pane).unwrap_or_default();
        self.lens_request(
            pane,
            block,
            plugin,
            lens_block(&command, &output, &folder, width),
        );
    }

    /// Sends `request` to the lens `plugin` on a worker; the picture comes back as [`Done::Drawn`].
    pub(in crate::hub) fn lens_request(
        &mut self,
        pane: PaneId,
        block: BlockId,
        plugin: Plugin,
        request: LensBlock,
    ) {
        self.plugins.lens.want(pane, block);
        let launcher = self.plugins.host.quick.clone();
        let done = self.plugins.host.sender();
        let started = self.plugins.host.run("temor-plugin-lens", move || {
            match launcher.lens(&plugin, &request) {
                Ok(drawing) => {
                    let _ = done.send(Done::Drawn {
                        pane,
                        block,
                        plugin: plugin.id().to_owned(),
                        drawing,
                    });
                }
                Err(error) => log_trouble(&error),
            }
        });
        if started.is_err() {
            self.plugins.lens.arrived(pane, block);
        }
    }

    /// The picture a lens drew for the block, if one did.
    pub fn lens_image_of(&self, pane: PaneId, block: BlockId) -> Option<LensImage> {
        self.plugins.lens.image(pane, block).cloned()
    }

    /// Takes the block's picture away.
    pub fn lens_hide(&mut self, pane: PaneId, block: BlockId) {
        self.plugins.lens.remove(pane, block);
    }

    /// The agent in `pane` changed from `was` to `is`: the adapter for the old one stops and the
    /// one for the new one starts, if a plugin adapts it.
    pub(in crate::hub) fn plugin_adapter_follows(
        &mut self,
        pane: PaneId,
        was: Option<String>,
        is: Option<String>,
    ) {
        match follow(was.as_deref(), is.as_deref()) {
            Follow::Stay => {}
            Follow::Stop => self.plugin_adapter_stop(pane),
            Follow::Restart(agent) => {
                self.plugin_adapter_stop(pane);
                self.plugin_adapter_start(pane, &agent);
            }
        }
    }

    fn plugin_adapter_stop(&mut self, pane: PaneId) {
        if let Some(running) = self.plugins.adapters.remove(&pane) {
            running.stop.begin();
        }
    }

    /// Starts the adapter plugin for `agent` in `pane`'s folder, if there is one. What it
    /// reports reaches the pane as a status line, like a program's own.
    pub(in crate::hub) fn plugin_adapter_start(&mut self, pane: PaneId, agent: &str) {
        let Some(plugin) = self.plugins.host.catalog.adapter(agent) else {
            return;
        };
        let Some(folder) = self.pane_folder(pane) else {
            return;
        };
        self.plugins.next_run += 1;
        let run = self.plugins.next_run;
        let stop = Generations::new();
        let mine = stop.begin();
        self.plugins.adapters.insert(
            pane,
            Running {
                run,
                stop: stop.clone(),
            },
        );
        let launcher = self.plugins.host.patient.clone();
        let (events, done) = (self.events.clone(), self.plugins.host.sender());
        let agent = agent.to_owned();
        let started = self.plugins.host.run("temor-plugin-adapter", move || {
            let mut seq = unix_millis();
            let result = launcher.adapt(
                &plugin,
                &agent,
                &folder,
                &|| stop.check(mine),
                &mut |phase, text| {
                    seq += 1;
                    let line = status_line(seq, phase, &agent, &text);
                    let _ = events.send((pane, PaneEvent::Reported(line)));
                },
            );
            if let Err(error) = result {
                log_trouble(&error);
            }
            let _ = done.send(Done::AdapterEnded { pane, run });
        });
        if started.is_err() {
            self.plugins.adapters.remove(&pane);
        }
    }
}

impl HubRef {
    /// The lenses that can draw the block.
    pub fn block_lenses(&self, pane: PaneId, block: BlockId) -> Vec<LensOffer> {
        self.0.borrow().block_lenses(pane, block)
    }

    /// The picture a lens drew for the block.
    pub fn lens_image_of(&self, pane: PaneId, block: BlockId) -> Option<LensImage> {
        self.0.borrow().lens_image_of(pane, block)
    }

    /// The pictures lenses drew in the pane, as the window draws them.
    pub fn lens_pictures(&self, pane: PaneId) -> Vec<LensPicture> {
        self.0.borrow().plugins.lens.pictures(pane)
    }

    /// Whether a lens drew a picture for the block (the menu then offers to hide it).
    pub fn lens_shown(&self, pane: PaneId, block: BlockId) -> bool {
        self.0.borrow().plugins.lens.image(pane, block).is_some()
    }
}

impl Desk {
    /// Asks a lens to draw a block, and draws the window again when it has.
    pub fn lens_run(&self, pane: PaneId, block: BlockId, lens: &str, width: u32) {
        self.settle(|hub| hub.lens_run(pane, block, lens, width));
    }

    /// Takes a block's picture away, and draws the window again.
    pub fn lens_hide(&self, pane: PaneId, block: BlockId) {
        self.settle(|hub| hub.lens_hide(pane, block));
    }
}

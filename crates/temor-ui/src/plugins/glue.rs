//! Plugins in the hub (PLAN P11). Mounted from `hub/mod.rs` as a child module, so it reaches the
//! hub's fields. Every plugin request runs on a worker thread; its answer comes back as a
//! [`Done`] and is applied here, on the window's thread. A plugin that fails shows nothing and is
//! logged (`log_trouble`). Lenses and adapters are in `glue_work.rs`.

use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::PluginId;
use temor_plugins::message::{Choice, Item};
use temor_plugins::{FOLDER_TABLE, Plugin, Routed, effect_of};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::plugins::effects::carry_out;
use crate::plugins::facts::{PluginFact, from_plugin, summary};
use crate::plugins::hands::DesktopHands;
use crate::plugins::host::{Done, log_trouble};
use crate::plugins::lens::Wanted;

#[path = "glue_work.rs"]
mod work;

impl Hub {
    /// Starts every `row.facts` plugin reading `dir`, the folder of `pane` on this computer. One
    /// read at a time for a pane: a folder that changed meanwhile is read again when it ends.
    pub(super) fn plugin_facts_start(&mut self, pane: PaneId, dir: &str) {
        let sources = self.plugins.host.catalog.facts_sources(FOLDER_TABLE);
        if sources.is_empty() || !self.plugins.reading.insert(pane) {
            return;
        }
        let launcher = self.plugins.host.quick.clone();
        let done = self.plugins.host.sender();
        let folder = dir.to_owned();
        let started = self.plugins.host.run("temor-plugin-facts", move || {
            let mut facts = Vec::new();
            for plugin in &sources {
                match launcher.facts(plugin, FOLDER_TABLE, &folder) {
                    Ok(given) => facts.extend(from_plugin(plugin.name(), given)),
                    Err(error) => log_trouble(&error),
                }
            }
            // A closed channel means the window is gone.
            let _ = done.send(Done::Facts {
                pane,
                folder,
                facts,
            });
        });
        if started.is_err() {
            self.plugins.reading.remove(&pane);
        }
    }

    /// The facts plugins last gave for the pane's folder (none before the first read, and none
    /// from a plugin that failed).
    pub fn plugin_facts_of(&self, pane: PaneId) -> Vec<PluginFact> {
        self.plugins.facts.get(&pane).cloned().unwrap_or_default()
    }

    /// The facts as the sidebar row says them (`cluster prod · region eu`).
    pub(super) fn plugin_facts_summary(&self, pane: PaneId) -> Option<String> {
        summary(&self.plugin_facts_of(pane))
    }

    /// Applies one finished piece of work.
    pub(super) fn plugin_done(&mut self, done: Done) {
        match done {
            Done::Facts {
                pane,
                folder,
                facts,
            } => self.plugin_facts_ready(pane, &folder, facts),
            Done::Rows {
                generation,
                query,
                plugin,
                items,
            } => self.plugin_rows_ready(generation, query, &plugin, items),
            Done::Chosen { effect } => {
                let note = match self.plugins.hands.clone() {
                    Some(hands) => carry_out(&effect, &*hands),
                    None => {
                        let hands = DesktopHands {
                            links: &*self.ssh.opener,
                            files: &*self.ssh.file_opener,
                        };
                        carry_out(&effect, &hands)
                    }
                };
                self.notices.extend(note);
            }
            Done::Drawn {
                pane,
                block,
                plugin,
                drawing,
            } => {
                if self.plugins.lens.arrived(pane, block) == Wanted::Yes {
                    self.plugins.lens.put(pane, block, plugin, drawing);
                }
            }
            Done::AdapterEnded { pane, run } => {
                if self
                    .plugins
                    .adapters
                    .get(&pane)
                    .is_some_and(|r| r.run == run)
                {
                    self.plugins.adapters.remove(&pane);
                }
            }
        }
    }

    fn plugin_facts_ready(&mut self, pane: PaneId, folder: &str, facts: Vec<PluginFact>) {
        // A pane that went away while its folder was read is no longer reading.
        if !self.plugins.reading.remove(&pane) {
            return;
        }
        match facts.is_empty() {
            true => self.plugins.facts.remove(&pane),
            false => self.plugins.facts.insert(pane, facts),
        };
        // The folder moved while the plugins were reading it: read the new one.
        if self.host_of(pane).is_none()
            && let Some(now) = self.pane_folder(pane)
            && now != folder
        {
            self.plugin_facts_start(pane, &now);
        }
    }

    /// The person typed `query` in the bar: every provider it routes to is asked, and the ones
    /// asked for the text before are told to stop.
    pub(super) fn plugins_ask(&mut self, query: &str) {
        let generation = self.plugins.host.keys.begin();
        self.plugins.offers.clear();
        for routed in self.plugins.host.catalog.providers(query) {
            let launcher = self.plugins.host.patient.clone();
            let keys = self.plugins.host.keys.clone();
            let done = self.plugins.host.sender();
            let typed = query.to_owned();
            // A worker that cannot start offers no rows; nothing else is waiting on it.
            let _ = self.plugins.host.run("temor-plugin-query", move || {
                let Routed { plugin, text } = routed;
                let result = launcher.query(
                    &plugin,
                    &text,
                    generation,
                    &|| keys.check(generation),
                    &mut |batch| {
                        let _ = done.send(Done::Rows {
                            generation,
                            query: typed.clone(),
                            plugin: plugin.clone(),
                            items: batch.items,
                        });
                    },
                );
                if let Err(error) = result {
                    log_trouble(&error);
                }
            });
        }
    }

    /// The bar closed or opened afresh: queries still streaming are told to stop and the rows
    /// they offered are forgotten.
    pub(super) fn plugins_stop(&mut self) {
        self.plugins.host.keys.begin();
        self.plugins.offers.clear();
    }

    fn plugin_rows_ready(
        &mut self,
        generation: u64,
        query: String,
        plugin: &Plugin,
        items: Vec<Item>,
    ) {
        if generation != self.plugins.host.keys.newest() {
            return;
        }
        self.plugins.offers.add(plugin, items);
        let rows = self.plugins.offers.plugin_rows();
        self.bar_apply(|model, cx| model.provider_rows(query, rows, cx));
    }

    /// The person chose a plugin's row: the plugin says what that does, and the bar's own
    /// actions (copy, open, nothing) carry it out.
    pub(super) fn plugin_pick(&mut self, plugin: PluginId, item: String) {
        let Some(offered) = self.plugins.offers.find(plugin.as_str(), &item).cloned() else {
            return;
        };
        let launcher = self.plugins.host.quick.clone();
        let done = self.plugins.host.sender();
        let _ = self.plugins.host.run("temor-plugin-choose", move || {
            let answer = launcher.activate(&offered.plugin, &offered.item, &Choice::Primary);
            match answer.map(effect_of) {
                Ok(Ok(effect)) => {
                    let _ = done.send(Done::Chosen { effect });
                }
                Ok(Err(refused)) => {
                    eprintln!(
                        "temor: the plugin {} was refused: {refused}",
                        offered.plugin.id()
                    );
                }
                Err(error) => log_trouble(&error),
            }
        });
    }
}

impl HubRef {
    /// The facts plugins gave for the pane's folder.
    pub fn plugin_facts_of(&self, pane: PaneId) -> Vec<PluginFact> {
        self.0.borrow().plugin_facts_of(pane)
    }
}

/// Carries finished plugin work into the hub for as long as the window lives, and draws the
/// window again after each. Call once, next to the facts loop.
pub fn spawn_plugins_loop(desk: Desk) {
    use dioxus::prelude::spawn;
    let Some(mut inbox) = desk.hub.0.borrow_mut().plugins.host.take_inbox() else {
        return;
    };
    spawn(async move {
        while let Some(done) = inbox.recv().await {
            desk.settle(|hub| hub.plugin_done(done));
        }
    });
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;

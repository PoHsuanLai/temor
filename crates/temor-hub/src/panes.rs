//! What the hub keeps of each pane, in one record per pane: closing the pane drops the record,
//! and everything in it goes with it (PLAN §4c, "one owner per piece of state").

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use temor_client::model::vocab::ids::PaneId;
use temor_ui_base::slot::Slot;

use super::facts::PaneFacts;
use super::history::PaneHistory;
use super::offer::PaneOffer;
use super::remote::{PaneRemote, RemotePane};
use super::run::Found;
use super::ssh::HostPane;
use super::viewers::PaneViewer;
use crate::plugins::state::PanePlugins;

/// What the hub keeps of one live pane. Made by the first part of the hub that hears of the pane,
/// dropped whole when the pane closes ([`Hub::drop_pane`](super::model::Hub)).
#[derive(Default)]
pub(super) struct PaneRecord {
    /// Its terminal, once it has one (a viewer pane never does).
    pub slot: Option<Rc<RefCell<Slot>>>,
    /// The program it runs in place of the shell (`temor -e`), kept for Restart.
    pub run: Option<Found>,
    /// Its session in history: the one it goes on with until its shell says where it is, then
    /// the one it began.
    pub history: PaneHistory,
    /// The shell offer in it.
    pub offer: PaneOffer,
    /// What it keeps while it is a viewer.
    pub viewer: Option<PaneViewer>,
    /// What plugins have given it: its folder's facts, lens pictures, its agent's adapter.
    pub plugins: PanePlugins,
    /// What it keeps while it is a session on another computer.
    pub host: Option<HostPane>,
    /// Its session on a helper, or in this computer's own daemon.
    pub remote: PaneRemote,
    /// What was last read of its folder, and the rule that says when to read it again.
    pub facts: PaneFacts,
}

/// The live panes' records.
#[derive(Default)]
pub(super) struct Panes(BTreeMap<PaneId, PaneRecord>);

impl Panes {
    /// The record of `pane`, if it is live.
    pub fn get(&self, pane: PaneId) -> Option<&PaneRecord> {
        self.0.get(&pane)
    }

    /// The record of `pane`, to change, if it is live.
    pub fn get_mut(&mut self, pane: PaneId) -> Option<&mut PaneRecord> {
        self.0.get_mut(&pane)
    }

    /// The record of `pane`, made empty if it has none yet.
    pub fn record(&mut self, pane: PaneId) -> &mut PaneRecord {
        self.0.entry(pane).or_default()
    }

    /// Drops `pane`'s record: the pane closed.
    pub fn remove(&mut self, pane: PaneId) -> Option<PaneRecord> {
        self.0.remove(&pane)
    }

    /// Every live pane's record.
    pub fn iter(&self) -> impl Iterator<Item = (PaneId, &PaneRecord)> {
        self.0.iter().map(|(pane, record)| (*pane, record))
    }

    /// How many panes are live.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `pane`'s terminal, if it is live and has one.
    pub fn slot(&self, pane: PaneId) -> Option<&Rc<RefCell<Slot>>> {
        self.get(pane)?.slot.as_ref()
    }

    /// What `pane` keeps while it is a session on another computer.
    pub fn host(&self, pane: PaneId) -> Option<&HostPane> {
        self.get(pane)?.host.as_ref()
    }

    /// What `pane` keeps while it is a session on another computer, to change.
    pub fn host_mut(&mut self, pane: PaneId) -> Option<&mut HostPane> {
        self.get_mut(pane)?.host.as_mut()
    }

    /// Every pane that is a session on another computer.
    pub fn hosts(&self) -> impl Iterator<Item = (PaneId, &HostPane)> {
        self.iter()
            .filter_map(|(pane, record)| Some((pane, record.host.as_ref()?)))
    }

    /// The panes alive on `alias`.
    pub fn on_host(&self, alias: &temor_client::ssh::HostAlias) -> Vec<PaneId> {
        self.hosts()
            .filter(|(_, host)| &host.alias == alias)
            .map(|(pane, _)| pane)
            .collect()
    }

    /// `pane`'s session on a helper, while it has one.
    pub fn remote(&self, pane: PaneId) -> Option<&RemotePane> {
        self.get(pane)?.remote.live.as_ref()
    }

    /// Every pane's session on a helper.
    pub fn remotes(&self) -> impl Iterator<Item = (PaneId, &RemotePane)> {
        self.iter()
            .filter_map(|(pane, record)| Some((pane, record.remote.live.as_ref()?)))
    }

    /// Every pane's session on a helper, to change.
    pub fn remotes_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut RemotePane)> {
        self.0
            .iter_mut()
            .filter_map(|(pane, record)| Some((*pane, record.remote.live.as_mut()?)))
    }

    /// Every live terminal, with its pane.
    pub fn slots(&self) -> impl Iterator<Item = (PaneId, &Rc<RefCell<Slot>>)> {
        self.iter()
            .filter_map(|(pane, record)| Some((pane, record.slot.as_ref()?)))
    }
}

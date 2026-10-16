//! What the hub keeps of its plugins.

use std::collections::{BTreeMap, BTreeSet};

use std::rc::Rc;

use temor_core::vocab::ids::PaneId;
use temor_plugins::Generations;

use super::effects::Hands;
use super::facts::PluginFact;
use super::host::PluginHost;
use super::lens::LensBook;
use super::offer::OfferBook;
use crate::view::launch::Launch;

/// A running adapter: which run it is, and the switch that stops it.
#[derive(Debug, Clone)]
pub struct Running {
    /// Told from an older run of the same pane's adapter.
    pub run: u64,
    /// Starting a newer count stops the adapter.
    pub stop: Generations,
}

/// Everything plugins have given the window, and the way to ask for more.
#[derive(Debug)]
pub struct PluginHub {
    /// The catalog, the launchers and the channel.
    pub host: PluginHost,
    /// The facts plugins last gave for each pane's folder.
    pub facts: BTreeMap<PaneId, Vec<PluginFact>>,
    /// The panes whose folder is being read by plugins now.
    pub reading: BTreeSet<PaneId>,
    /// The rows plugins offered for the text in the bar.
    pub offers: OfferBook,
    /// The pictures lenses drew.
    pub lens: LensBook,
    /// The adapters running, by pane.
    pub adapters: BTreeMap<PaneId, Running>,
    /// The number of the next adapter run.
    pub next_run: u64,
    /// What carries out a chosen row; the desktop's when none is set (tests set a fake).
    pub hands: Option<Rc<dyn Hands>>,
}

impl PluginHub {
    /// The plugins of the person who started the window.
    pub fn new(launch: &Launch) -> PluginHub {
        PluginHub::over(PluginHost::new(launch))
    }

    /// A hub over `host` (tests build one over a scratch catalog).
    pub fn over(host: PluginHost) -> PluginHub {
        PluginHub {
            host,
            facts: BTreeMap::new(),
            reading: BTreeSet::new(),
            offers: OfferBook::default(),
            lens: LensBook::default(),
            adapters: BTreeMap::new(),
            next_run: 0,
            hands: None,
        }
    }

    /// Forgets everything about a pane that is gone, and stops its adapter.
    pub fn forget_pane(&mut self, pane: PaneId) {
        self.facts.remove(&pane);
        self.reading.remove(&pane);
        self.lens.forget_pane(pane);
        if let Some(running) = self.adapters.remove(&pane) {
            running.stop.begin();
        }
    }
}

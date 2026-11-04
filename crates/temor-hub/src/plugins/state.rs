//! What the hub keeps of its plugins.

use std::rc::Rc;

use temor_client::plugins::Generations;

use super::effects::Hands;
use super::facts::PluginFact;
use super::host::PluginHost;
use super::lens::LensBook;
use super::offer::OfferBook;
use temor_ui_base::launch::Launch;

/// A running adapter: which run it is, and the switch that stops it.
#[derive(Debug, Clone)]
pub struct Running {
    /// Told from an older run of the same pane's adapter.
    pub run: u64,
    /// Starting a newer count stops the adapter.
    pub stop: Generations,
}

/// What plugins have given one pane, kept in its record.
#[derive(Debug, Default)]
pub struct PanePlugins {
    /// The facts plugins last gave for its folder.
    pub facts: Vec<PluginFact>,
    /// Whether plugins are reading its folder now.
    pub reading: bool,
    /// The pictures lenses drew for its blocks.
    pub lens: LensBook,
    /// The adapter running for its agent.
    pub adapter: Option<Running>,
}

/// What plugins have given the window as a whole, and the way to ask for more. What they gave
/// each pane is in the pane's record ([`PanePlugins`]).
#[derive(Debug)]
pub struct PluginHub {
    /// The catalog, the launchers and the channel.
    pub host: PluginHost,
    /// The rows plugins offered for the text in the bar.
    pub offers: OfferBook,
    /// The number of the next adapter run.
    pub next_run: u64,
    /// What carries out a chosen row; the desktop's when none is set (tests set a fake).
    pub hands: Option<Rc<dyn Hands>>,
}

impl PluginHub {
    /// The plugins of the person who started the window.
    pub fn new(launch: &Launch) -> PluginHub {
        PluginHub::over(PluginHost::new(&launch.ambient.plugin_dirs()))
    }

    /// A hub over `host` (tests build one over a scratch catalog).
    pub fn over(host: PluginHost) -> PluginHub {
        PluginHub {
            host,
            offers: OfferBook::default(),
            next_run: 0,
            hands: None,
        }
    }
}

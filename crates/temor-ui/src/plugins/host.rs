//! The plugins the window has, and the way their answers come back.

use std::sync::Arc;

use temor_core::vocab::ids::{BlockId, PaneId};
use temor_plugins::message::Item;
use temor_plugins::{
    CancelKind, Catalog, Drawing, Effect, Generations, Launcher, Plugin, PluginError, Waits,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::facts::PluginFact;
use crate::view::launch::Launch;

/// A finished piece of work, sent from a worker thread to the window.
#[derive(Debug)]
pub enum Done {
    /// A pane's folder was read by every plugin that knows folders.
    Facts {
        /// The pane.
        pane: PaneId,
        /// The folder that was read.
        folder: String,
        /// What the plugins said, in serving order.
        facts: Vec<PluginFact>,
    },
    /// Some rows for the bar's query.
    Rows {
        /// The keystroke they answer.
        generation: u64,
        /// The text typed, whole.
        query: String,
        /// Who offered them (boxed: a plugin's description is many times the other answers).
        plugin: Box<Plugin>,
        /// The rows.
        items: Vec<Item>,
    },
    /// The row the person chose was answered.
    Chosen {
        /// What to do, already checked.
        effect: Effect,
    },
    /// A lens drew a block.
    Drawn {
        /// The pane.
        pane: PaneId,
        /// The block.
        block: BlockId,
        /// The lens's id.
        plugin: String,
        /// The picture.
        drawing: Drawing,
    },
    /// An adapter's run ended (it finished, failed or was stopped).
    AdapterEnded {
        /// The pane.
        pane: PaneId,
        /// Which run, so a late message from an old run is told from the new one.
        run: u64,
    },
}

/// The catalog found at start, the launchers that run its plugins, and the channel.
pub struct PluginHost {
    pub(crate) catalog: Arc<Catalog>,
    /// For answers that should be quick: facts, a chosen row, a lens.
    pub(crate) quick: Launcher,
    /// For streams: the bar's query and an adapter.
    pub(crate) patient: Launcher,
    /// The bar's keystrokes: a newer one cancels the older queries.
    pub(crate) keys: Generations,
    done: UnboundedSender<Done>,
    inbox: Option<UnboundedReceiver<Done>>,
}

impl std::fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHost")
            .field("plugins", &self.catalog.plugins().len())
            .finish_non_exhaustive()
    }
}

fn log(line: &str) {
    eprintln!("{line}");
}

impl PluginHost {
    /// The plugins installed for the person of `launch`: `<data dir>/temor/plugins`, their own
    /// directory first, then the system's. A manifest or program that cannot be used is logged
    /// once, here.
    pub fn new(launch: &Launch) -> PluginHost {
        let catalog = Catalog::discover(&launch.ambient.plugin_dirs());
        for problem in catalog.problems() {
            eprintln!("temor: plugin set aside: {problem}");
        }
        PluginHost::with_catalog(catalog)
    }

    /// A host over `catalog` (tests build one over a scratch folder).
    pub fn with_catalog(catalog: Catalog) -> PluginHost {
        let (done, inbox) = unbounded_channel();
        PluginHost {
            catalog: Arc::new(catalog),
            quick: Launcher::new(Waits::quick(), log),
            patient: Launcher::new(Waits::default(), log),
            keys: Generations::new(),
            done,
            inbox: Some(inbox),
        }
    }

    /// The window's loop takes the receiving end once.
    pub fn take_inbox(&mut self) -> Option<UnboundedReceiver<Done>> {
        self.inbox.take()
    }

    /// A way back to the window for a worker.
    pub(crate) fn sender(&self) -> UnboundedSender<Done> {
        self.done.clone()
    }

    /// Runs `work` on a thread of its own. If the thread cannot start the work never happens,
    /// which for a plugin is the same as one that shows nothing; the caller hears of it.
    pub(crate) fn run(
        &self,
        name: &str,
        work: impl FnOnce() + Send + 'static,
    ) -> std::io::Result<()> {
        let started = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(work);
        if let Err(error) = &started {
            eprintln!("temor: plugin work could not start: {error}");
        }
        started.map(|_| ())
    }
}

/// Logs why a plugin gave no answer, unless it was stopped on purpose.
pub(crate) fn log_trouble(error: &PluginError) {
    if error.kind() == CancelKind::Trouble {
        eprintln!("temor: {error}");
    }
}

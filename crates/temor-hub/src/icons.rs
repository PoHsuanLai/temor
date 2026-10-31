//! The rows' icons in the window (PLAN P22): the client's `icons` finds the apps and folders'
//! icons on a thread of its own and the icon pack's brands; this carries the word that a look-up
//! is done back to the window, which draws the rows again.

use std::path::Path;

use temor_client::icons::{IconPack, Icons};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// The window's icons: the look-ups, the pack, and the word that one is done.
#[derive(Debug)]
pub struct IconsHub {
    pub(super) icons: Icons,
    pub(super) pack: IconPack,
    pub(super) done: Option<UnboundedReceiver<()>>,
}

impl IconsHub {
    /// The icons of a window, with the pack in `icons.toml` under `config_dir` and what was
    /// wrong with it, to tell the person.
    pub(super) fn new(config_dir: &Path, home: &Path) -> (IconsHub, Vec<String>) {
        let (pack, trouble) = IconPack::load(config_dir, home);
        let (told, done) = unbounded_channel();
        let icons = Icons::new(move || {
            // A closed channel means the window is gone.
            let _ = told.send(());
        });
        let hub = IconsHub {
            icons,
            pack,
            done: Some(done),
        };
        (hub, trouble)
    }
}

//! Opening a pane: the engine started on a program (or fed from elsewhere), its I/O on a thread
//! of its own, and the pane's core around it.

use std::path::PathBuf;
use std::sync::Arc;

use temor_core::notify::{Focus, Limiter};
use temor_pane::settings::NotifyAfter;
use temor_pane::{
    EventSink, FedSpec, Feeder, Hold, PaneError, PaneEvent, PaneHandle, PaneSpec, start, start_fed,
    start_restored,
};
use temor_shell::report::{KEY_VAR, ReportKey};
use temor_vt::Snapshot;

use super::blocks::BlockLog;
use super::clock::Clock;
use super::notify::Notifier;
use super::sources::Sources;
use super::{Lifetime, PaneCore};

/// What a pane is made from besides its engine.
pub struct PaneKit {
    /// When a finished command is worth a notice.
    pub notify_after: NotifyAfter,
    pub notifier: Box<dyn Notifier>,
    pub clock: Box<dyn Clock>,
    /// Where "Save output" places files (the downloads shelf).
    pub shelf_dir: PathBuf,
}

impl std::fmt::Debug for PaneKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneKit")
            .field("shelf_dir", &self.shelf_dir)
            .finish_non_exhaustive()
    }
}

/// Why a pane could not open.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The pane engine could not start (see `temor-pane`).
    #[error(transparent)]
    Pane(#[from] PaneError),
    /// The pane's I/O thread could not start.
    #[error("the terminal could not start its background work: {0}")]
    Thread(#[source] std::io::Error),
}

impl PaneCore {
    /// Starts the program and its pane, its events going to `sink`. With a `snapshot` the
    /// terminal begins as that closed pane's screen and scrollback, the new program writing below.
    pub fn start(
        spec: PaneSpec,
        sink: Arc<dyn EventSink>,
        snapshot: Option<&Snapshot>,
        kit: PaneKit,
    ) -> Result<Self, OpenError> {
        // Only temor sets the key in a child's environment (temor-pty's contract).
        let report_key = spec
            .spawn
            .env
            .get(KEY_VAR)
            .and_then(|key| key.to_str())
            .and_then(ReportKey::parse);
        let program = spec.spawn.program.clone();
        let heard = Arc::clone(&sink);
        let (pane, runner) = match snapshot {
            Some(snapshot) => start_restored(spec, snapshot, sink)?,
            None => start(spec, sink)?,
        };
        std::thread::Builder::new()
            .name("temor-pane-io".to_owned())
            .spawn(move || runner.run())
            .map_err(OpenError::Thread)?;
        let mut core = Self::around(pane, kit);
        core.report_key = report_key;
        let deliver: temor_shell::suggest::Deliver =
            Arc::new(move |answered| heard.send(PaneEvent::Suggested(answered)));
        core.sources = Sources::for_program(&program, deliver);
        Ok(core)
    }

    /// Starts a pane whose program runs elsewhere (a host's helper), its events going to `sink`.
    /// The returned feeder is how the far end's output reaches the pane.
    pub fn start_fed(
        spec: FedSpec,
        sink: Arc<dyn EventSink>,
        kit: PaneKit,
    ) -> Result<(Self, Feeder), OpenError> {
        let (pane, runner, feeder) = start_fed(spec, sink)?;
        std::thread::Builder::new()
            .name("temor-pane-io".to_owned())
            .spawn(move || runner.run())
            .map_err(OpenError::Thread)?;
        let mut core = Self::around(pane, kit);
        core.site = temor_core::shelf::PaneSite::Remote;
        Ok((core, feeder))
    }

    /// The core of a pane whose engine has started.
    fn around(pane: PaneHandle, kit: PaneKit) -> Self {
        let clip_mirror = pane.with_term(|term| term.clipboard());
        Self {
            pane: Arc::new(pane),
            title: String::new(),
            lifetime: Lifetime::Running,
            hold: Hold::Released,
            hold_generation: 0,
            blocks: BlockLog::default(),
            shell_state: temor_core::session::ShellState::default(),
            shell_path: None,
            inherited: Vec::new(),
            report_key: None,
            sources: Sources::default(),
            folder: None,
            cwd: None,
            progress: None,
            notify_after: kit.notify_after,
            limiter: Limiter::default(),
            notifier: kit.notifier,
            clock: kit.clock,
            shelf_dir: kit.shelf_dir,
            quoting: temor_core::shelf::Quoting::Posix,
            site: temor_core::shelf::PaneSite::Local,
            clip_write: temor_core::remote::Grant::Ask,
            clip_mirror,
            clip_pending: None,
            focus: Focus::Focused,
            sent: None,
            said: None,
        }
    }
}

//! Opening a session: the pane engine started on a program (or fed from elsewhere), and the
//! session's state around it.

use std::sync::Arc;

use temor_core::notify::{Focus, Limiter};
use temor_pane::{
    EventSink, Feeder, Hold, PaneEvent, PaneHandle, start, start_fed, start_restored,
};
use temor_render::{PixelSize, Scale};
use temor_vt::Snapshot;
use tokio::sync::mpsc::UnboundedSender;

use super::Session;
use super::blocks::BlockLog;
use super::model::{FedParts, Lifetime, SessionKit, SessionParts};
use crate::error::UiError;
use crate::paint::Painter;
use crate::pane::UiSink;

impl Session {
    /// Starts the program and its session. Events for the window go to `events`.
    pub fn open(parts: SessionParts, events: UnboundedSender<PaneEvent>) -> Result<Self, UiError> {
        Self::open_with(parts, Arc::new(UiSink(events)), None)
    }

    /// Starts the program and its session, its events going to `sink`. With a `snapshot` the
    /// terminal begins as that closed pane's screen and scrollback, the new program writing below.
    pub fn open_with(
        parts: SessionParts,
        sink: Arc<dyn EventSink>,
        snapshot: Option<&Snapshot>,
    ) -> Result<Self, UiError> {
        let SessionParts {
            spec,
            fonts,
            keymap,
            clipboard,
            opener,
            notify_after,
            notifier,
            clock,
            shelf_dir,
        } = parts;
        let painter = Painter::new(fonts, spec.palette)?;
        let (pane, runner) = match snapshot {
            Some(snapshot) => start_restored(spec, snapshot, sink)?,
            None => start(spec, sink)?,
        };
        std::thread::Builder::new()
            .name("temor-pane-io".to_owned())
            .spawn(move || runner.run())
            .map_err(UiError::Thread)?;
        let kit = SessionKit {
            keymap,
            clipboard,
            opener,
            notify_after,
            notifier,
            clock,
            shelf_dir,
        };
        Ok(Self::assemble(pane, painter, kit))
    }

    /// Starts a session whose program runs elsewhere (a host's helper), its events going to
    /// `sink`. The returned feeder is how the far end's output reaches the pane.
    pub fn open_fed(parts: FedParts, sink: Arc<dyn EventSink>) -> Result<(Self, Feeder), UiError> {
        let FedParts { spec, fonts, kit } = parts;
        let painter = Painter::new(fonts, spec.palette)?;
        let (pane, runner, feeder) = start_fed(spec, sink)?;
        std::thread::Builder::new()
            .name("temor-pane-io".to_owned())
            .spawn(move || runner.run())
            .map_err(UiError::Thread)?;
        let mut session = Self::assemble(pane, painter, kit);
        session.site = temor_core::shelf::PaneSite::Remote;
        Ok((session, feeder))
    }

    fn assemble(pane: PaneHandle, painter: Painter, kit: SessionKit) -> Self {
        let clip_mirror = pane.with_term(|term| term.clipboard());
        Self {
            pane,
            painter,
            keymap: kit.keymap,
            clipboard: kit.clipboard,
            opener: kit.opener,
            fit: None,
            scale: Scale::new(1.0),
            pixels: PixelSize::new(0, 0),
            selection: None,
            drag: None,
            click: None,
            finder: None,
            pending: None,
            title: String::new(),
            lifetime: Lifetime::Running,
            notice: None,
            hold: Hold::Released,
            hold_generation: 0,
            find_text: String::new(),
            pending_query: None,
            blocks: BlockLog::default(),
            selected_block: None,
            tools_height: super::ToolsHeight::default(),
            pointer_over: None,
            cwd: None,
            progress: None,
            focus: Focus::Focused,
            notify_after: kit.notify_after,
            limiter: Limiter::default(),
            notifier: kit.notifier,
            clock: kit.clock,
            bell_until: None,
            shelf_dir: kit.shelf_dir,
            asked: None,
            view_ask: None,
            quoting: temor_core::shelf::Quoting::Posix,
            site: temor_core::shelf::PaneSite::Local,
            clip_write: temor_core::remote::Grant::Ask,
            clip_mirror,
            clip_pending: None,
        }
    }
}

//! Opening a session: the pane's core opened by the client (`PaneCore::start`), and the
//! window's state around it.

use std::sync::Arc;

use temor_panes::pane::{EventSink, Feeder, PaneEvent};
use temor_panes::render::{PixelSize, Scale};
use temor_panes::vt::Snapshot;
use tokio::sync::mpsc::UnboundedSender;

use super::Session;
use super::model::{FedParts, PaneCore, SessionParts, WindowKit};
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
        let SessionParts { spec, fonts, kit } = parts;
        let painter = Painter::new(fonts, spec.palette)?;
        let (core_kit, kit) = kit.split();
        let core = PaneCore::start(spec, sink, snapshot, core_kit)?;
        Ok(Self::assemble(core, painter, kit))
    }

    /// Starts a session whose program runs elsewhere (a host's helper), its events going to
    /// `sink`. The returned feeder is how the far end's output reaches the pane.
    pub fn open_fed(parts: FedParts, sink: Arc<dyn EventSink>) -> Result<(Self, Feeder), UiError> {
        let FedParts { spec, fonts, kit } = parts;
        let painter = Painter::new(fonts, spec.palette)?;
        let (core_kit, kit) = kit.split();
        let (core, feeder) = PaneCore::start_fed(spec, sink, core_kit)?;
        Ok((Self::assemble(core, painter, kit), feeder))
    }

    fn assemble(core: PaneCore, painter: Painter, kit: WindowKit) -> Self {
        let blink_from = core.clock.now();
        let pane = std::sync::Arc::clone(&core.pane);
        Self {
            core: temor_panes::share(core),
            pane,
            painter,
            keymap: kit.keymap,
            clipboard: kit.clipboard,
            opener: kit.opener,
            fit: None,
            layout: None,
            scale: Scale::new(1.0),
            pixels: PixelSize::new(0, 0),
            selection: None,
            drag: None,
            click: None,
            finder: None,
            pending: None,
            notice: None,
            find_text: String::new(),
            pending_query: None,
            ann: temor_client::model::session::Annotations::default(),
            pointer_over: None,
            link_hover: None,
            list_map: None,
            live_ground: None,
            composer: Default::default(),
            blink: super::blink::Blink {
                setting: kit.cursor_blink,
                from: blink_from,
                painted: temor_panes::render::BlinkPhase::On,
            },
            bell_until: None,
            asked: None,
            view_ask: None,
            kept: Vec::new(),
        }
    }
}

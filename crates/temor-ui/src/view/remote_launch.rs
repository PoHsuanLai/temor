//! Opening a session whose program runs on a host's helper (PLAN §9b, Tier B): the same window
//! parts as a session on this computer's terminal, with a [`PaneLink`] where the terminal would
//! be.

use std::sync::Arc;

use temor_core::vocab::ids::PaneId;
use temor_pane::settings::{CursorStyle, FontChoice};
use temor_pane::{FedSpec, Feeder, PaneEvent, PaneLink, TransferSetup};
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, Palette};
use temor_vt::{CellPx, CursorShape, GridSize, LineCount};
use tokio::sync::mpsc::UnboundedSender;

use super::launch::{DesktopClipboard, Launch, transfer_limits};
use crate::edge::NoopNotifier;
use crate::edge::SystemClock;
use crate::error::UiError;
use crate::pane::TaggedSink;
use crate::platform::default_opener;
use crate::session::{FedParts, Session, SessionKit};

/// Opens a session for pane `pane` at 80 by 24 cells (the first layout refits it) whose input
/// and size go to `link`. The returned feeder is how the host's output reaches the pane.
pub fn open_fed_session(
    launch: &Launch,
    palette: Palette,
    pane: PaneId,
    events: UnboundedSender<(PaneId, PaneEvent)>,
    link: Arc<dyn PaneLink>,
) -> Result<(Session, Feeder), UiError> {
    let settings = &launch.settings;
    let family = match &settings.font {
        FontChoice::SystemMonospace => FontFamily::Monospace,
        FontChoice::Named(name) => FontFamily::Named(name.clone()),
    };
    let fonts = FontSet::load(
        &FontConfig {
            family,
            size: FontSize::new(settings.font_size.points()),
        },
        FontSource::System,
    )?;
    let spec = FedSpec {
        link,
        grid: GridSize::new(80, 24),
        cell: CellPx::new(8, 16),
        scrollback: LineCount(settings.scrollback.lines() as usize),
        palette,
        cursor: match settings.cursor {
            CursorStyle::Block => CursorShape::Block,
            CursorStyle::Bar => CursorShape::Bar,
            CursorStyle::Underline => CursorShape::Underline,
        },
        transfers: Some(TransferSetup {
            staging: launch.ambient.staging_dir(pane.0),
            limits: transfer_limits(),
        }),
    };
    let kit = SessionKit {
        keymap: launch.keymap.clone(),
        clipboard: Box::new(DesktopClipboard),
        opener: default_opener(),
        notify_after: settings.notify_after,
        // TODO(quire-desktop): the desktop's notification service (quire v0.2.30 has none).
        notifier: Box::new(NoopNotifier),
        clock: Box::new(SystemClock::new()),
        shelf_dir: launch.ambient.shelf_dir(),
    };
    Session::open_fed(
        FedParts { spec, fonts, kit },
        Arc::new(TaggedSink::new(pane, events)),
    )
}

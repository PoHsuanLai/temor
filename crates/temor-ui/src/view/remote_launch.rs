//! Opening a session whose program runs on a host's helper (PLAN §9b, Tier B): the same window
//! parts as a session on this computer's terminal, with a [`PaneLink`] where the terminal would
//! be.

use std::sync::Arc;

use temor_core::vocab::ids::PaneId;
use temor_pane::{FedSpec, Feeder, PaneEvent, PaneLink, TransferSetup, cursor_shape};
use temor_render::Palette;
use temor_vt::LineCount;
use tokio::sync::mpsc::UnboundedSender;

use super::launch::{Launch, first_size, load_fonts, session_kit, transfer_limits};
use crate::error::UiError;
use crate::pane::TaggedSink;
use crate::session::{FedParts, Session};

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
    let fonts = load_fonts(&settings.font, settings.font_size)?;
    let (grid, cell) = first_size();
    let spec = FedSpec {
        link,
        grid,
        cell,
        scrollback: LineCount(settings.scrollback.lines() as usize),
        palette,
        cursor: cursor_shape(settings.cursor),
        transfers: Some(TransferSetup {
            staging: launch.ambient.staging_dir(pane.0),
            limits: transfer_limits(),
        }),
    };
    Session::open_fed(
        FedParts {
            spec,
            fonts,
            kit: session_kit(launch),
        },
        Arc::new(TaggedSink::new(pane, events)),
    )
}

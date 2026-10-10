//! What the window draws for a pane's blocks beside the grid (PLAN §9 blocks): the sticky header
//! (`block_header`), the toolbar on the hovered or selected block (`block_tools`), and the rest
//! of the pane's chrome (working folder, title, progress, bell). A gutter click still selects a
//! block; the cards lane's bands show how each one ended.

use temor_core::vocab::ids::BlockId;

use super::block_header::HeaderView;
use super::block_tools::ToolsView;
use super::status::{ProgressLine, progress_line};
use crate::session::Session;
use ds::base::text::clip::clip_chars;
use ds::prelude::Availability;

/// What the window draws beside the pane, read from the session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chrome {
    pub header: Option<HeaderView>,
    pub tools: Option<ToolsView>,
    pub cwd: Option<String>,
    pub title: String,
    pub progress: Option<ProgressLine>,
    pub bell: bool,
}

/// The toolbar for the first of `hovered` and the selected block that has a place on screen.
fn tools_of(
    session: &Session,
    hovered: Option<BlockId>,
    pad: f32,
    cell_h: f32,
) -> Option<ToolsView> {
    [hovered, session.selected_block()]
        .into_iter()
        .flatten()
        .find_map(|id| {
            let spot = session.tools_spot(id)?;
            Some(ToolsView {
                id,
                top: pad + spot.row as f32 * cell_h,
                inset: pad,
                summary: session.block_summary(id).unwrap_or_default(),
                output: match session.block_finished(id) {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                rerun: match session.block_command(id).is_some_and(|c| !c.is_empty()) {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
            })
        })
}

/// The chrome for a session, with `hovered` the block under the pointer.
pub fn chrome_of(session: &Session, hovered: Option<BlockId>) -> Chrome {
    let (pad, cell_h) = session.gutter_geometry().unwrap_or((0.0, 0.0));
    let columns = session.fit().map_or(80, |fit| usize::from(fit.grid.cols.0));
    let header = session.sticky_header().map(|header| HeaderView {
        id: header.id,
        command: clip_chars(&header.command, columns),
        inset: pad,
        height: pad + cell_h,
    });
    Chrome {
        header,
        tools: tools_of(session, hovered, pad, cell_h),
        cwd: session.cwd().map(str::to_owned),
        title: session.title().to_owned(),
        progress: session.progress().and_then(progress_line),
        bell: session.bell_showing(),
    }
}

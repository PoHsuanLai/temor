//! What the window draws for a pane's blocks beside the grid (PLAN §9 blocks): the sticky header
//! (`block_header`), how each card ended (`block_meta`), the tools on the hovered or selected
//! block (`block_tools`), and the pane's progress and bell. A gutter click still selects a block;
//! the cards lane's bands show how each one ended.

use temor_core::vocab::ids::BlockId;

use super::block_header::HeaderView;
use super::block_meta::{MetaView, header_row, metas_of};
use super::block_tools::ToolsView;
use super::status::{ProgressLine, progress_line};
use crate::session::{Gutter, Session};
use ds::base::text::clip::clip_chars;
use ds::prelude::Availability;

/// What the window draws beside the pane, read from the session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chrome {
    pub header: Option<HeaderView>,
    pub tools: Option<ToolsView>,
    /// How each card on screen ended, except the one whose tools say it themselves.
    pub metas: Vec<MetaView>,
    pub progress: Option<ProgressLine>,
    pub bell: bool,
}

/// The tools for the first of `hovered` and the selected block that has a place on screen. Their
/// meta is left out where it would reach the header's text; the buttons show whatever the room,
/// since the person asked for them.
fn tools_of(session: &Session, hovered: Option<BlockId>, gutter: Gutter) -> Option<ToolsView> {
    [hovered, session.selected_block()]
        .into_iter()
        .flatten()
        .find_map(|id| {
            let spot = session.tools_spot(id)?;
            Some(ToolsView {
                id,
                height: session.tools_height(),
                top: gutter.row_top(spot.row),
                row: gutter.cell_h,
                inset: gutter.side,
                meta: session.block_meta(id).filter(|words| {
                    let cols = session
                        .block_marks()
                        .into_iter()
                        .find(|mark| mark.id == id)
                        .map_or(0, |mark| mark.header_cols);
                    header_row(session, gutter).is_some_and(|row| row.fits(cols, words))
                }),
                output: match session.block_finished(id) {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                rerun: match session.block_command(id).is_some_and(|c| !c.is_empty()) {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                view: match session.block_finished(id) {
                    true => Availability::Enabled,
                    false => Availability::Disabled,
                },
                // The window knows which panes are on another computer.
                edit: None,
            })
        })
}

/// The chrome for a session. The hovered block is the session's own (it follows the wheel and
/// clears when the pointer leaves); `_hovered`, the window's signal of it, only makes the window
/// draw again when it changes.
pub fn chrome_of(session: &Session, _hovered: Option<BlockId>) -> Chrome {
    let gutter = session.gutter_geometry().unwrap_or(Gutter {
        side: 0.0,
        top: 0.0,
        cell_h: 0.0,
    });
    let columns = session.fit().map_or(80, |fit| usize::from(fit.grid.cols.0));
    let header = session.sticky_header().map(|header| HeaderView {
        id: header.id,
        command: clip_chars(&header.command, columns),
        inset: gutter.side,
        height: gutter.row_top(1),
    });
    let tools = tools_of(session, session.hovered_block(), gutter);
    let metas = metas_of(session, tools.as_ref().map(|tools| tools.id));
    Chrome {
        header,
        tools,
        metas,
        progress: session.progress().and_then(progress_line),
        bell: session.bell_showing(),
    }
}

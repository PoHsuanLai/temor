//! Giving a running pane to a workspace as a tab of its own: the end of "Move to Space". The
//! workspace step has no input for a pane that already exists, so this builds the same tab
//! `workspace_step` builds for a new one, around the pane that is already running.

use crate::error::CoreError;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::Cwd;
use crate::workspace::{Workspace, WorkspaceOut};

/// A copy of `ws` with a new tab around `pane`, in front, and what to tell the window. The
/// counters move past the ids in use. Refused, leaving `ws` as it is, when the pane is already
/// in a tab or no id is left.
pub fn adopt(
    ws: &Workspace,
    pane: PaneId,
    cwd: Option<Cwd>,
) -> Result<(Workspace, Vec<WorkspaceOut>), CoreError> {
    let mut ws = ws.clone();
    if ws.tab_of(pane).is_some() {
        return Err(CoreError::DuplicatePane(pane.0));
    }
    let next_tab = ws.next_tab.checked_add(1).ok_or(CoreError::IdsExhausted)?;
    let next_pane = ws
        .next_pane
        .max(pane.0.checked_add(1).ok_or(CoreError::IdsExhausted)?);
    let id = TabId(ws.next_tab);
    ws.next_tab = next_tab;
    ws.next_pane = next_pane;
    ws.open_shell_tab(id, pane, cwd);
    Ok((ws, vec![WorkspaceOut::Focused(pane), WorkspaceOut::Changed]))
}

//! "Move tab to Space" (PLAN §7): a tab leaves one Space's workspace and joins another's with
//! its panes untouched. The panes keep their ids and their running programs (the window's pane
//! slots are keyed by pane id and shared by every Space), so only the bookkeeping moves: the
//! tab, its focused pane and its panes' folders. Pure.

use crate::space::Tab;
use crate::workspace::{TabsIn, Workspace};
use temor_core::error::CoreError;
use temor_core::vocab::ids::TabId;

/// `from` without `tab`, and `to` with it at the end of its tabs (loose, with a new tab id from
/// `to`'s counter, in no folder); `to`'s front tab stays in front unless it had none. The pane
/// counter of `to` moves past the moved panes. Refused, changing nothing, when `tab` is not in
/// `from` or an id is used up. Moving a tab to the workspace it is in is the caller's to refuse.
pub fn carry_tab(
    from: &Workspace,
    tab: TabId,
    to: &Workspace,
) -> Result<(Workspace, Workspace), CoreError> {
    let moved = from
        .tabs
        .tab(tab)
        .cloned()
        .ok_or(CoreError::UnknownTab(tab.0))?;
    let panes = moved.tree.panes();
    let focus = from.focus_of(tab);
    let mut src = from.clone();
    let mut dst = to.clone();

    let id = TabId(dst.next_tab);
    dst.next_tab = dst.next_tab.checked_add(1).ok_or(CoreError::IdsExhausted)?;
    for pane in &panes {
        let past = pane.0.checked_add(1).ok_or(CoreError::IdsExhausted)?;
        dst.next_pane = dst.next_pane.max(past);
    }

    src.tabs_do(TabsIn::Remove(tab));
    src.focus.remove(&tab);
    for pane in &panes {
        if let Some(cwd) = src.cwds.remove(pane) {
            dst.cwds.insert(*pane, cwd);
        }
        if let Some(file) = src.viewers.remove(pane) {
            dst.viewers.insert(*pane, file);
        }
    }

    dst.tabs.place.tabs.push(Tab {
        id,
        title: moved.title,
        pin: moved.pin,
        folder: None,
        tree: moved.tree,
    });
    if let Some(pane) = focus {
        dst.focus.insert(id, pane);
    }
    if dst.tabs.place.focused.is_none() {
        dst.tabs.place.focused = Some(id);
    }
    Ok((src, dst))
}

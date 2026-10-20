//! Agents in the window: the menu on a tab's row and the sheet over the window (what a restore
//! would change, a worktree's findings, the leftover branches). The hub prepares the words; these
//! views draw them with quire's components and send the person's choices back.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::components::content::label::LabelStyle;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::{TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use crate::hub::{
    AgentMenu, AgentPick, Desk, PointLine, RestoreMenu, SheetView, WAIT_FOR_AGENT, Where,
    pin_as_shell,
};

/// The choices of a tab's menu as quire's menu items.
fn items_of(menu: &AgentMenu) -> Vec<MenuItem<AgentPick>> {
    let mut items = Vec::new();
    match &menu.restore {
        RestoreMenu::Absent => {}
        // A context menu leaves out a disabled item, so the reason is a line of its own.
        RestoreMenu::Waiting => items.push(MenuItem::Info {
            title: "Restore…".to_owned(),
            detail: Some(WAIT_FOR_AGENT.to_owned()),
        }),
        RestoreMenu::Lines(lines) => items.push(MenuItem::Submenu {
            title: "Restore…".to_owned(),
            image: None,
            availability: Availability::Enabled,
            children: lines
                .iter()
                .map(|line| match line {
                    PointLine::Choice { id, label } => {
                        MenuItem::new(AgentPick::Restore(*id), label.clone())
                    }
                    PointLine::Note(text) => MenuItem::Info {
                        title: text.clone(),
                        detail: None,
                    },
                })
                .collect(),
        }),
    }
    match menu.place {
        Where::Checkout => items.push(MenuItem::new(AgentPick::RunInWorktree, "Run in a worktree")),
        Where::Worktree => {
            items.push(MenuItem::new(AgentPick::Details, "Show worktree details"));
            items.push(MenuItem::new(AgentPick::MergeBack, "Merge back (Squash)"));
        }
    }
    items.push(MenuItem::Separator);
    items.push(MenuItem::new(AgentPick::Leftovers, "Leftover branches"));
    items
}

/// Whether a tab's menu pins it or lets it go: a row's pins, a pinned tile's unpins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinAct {
    Pin,
    Unpin,
}

/// What a tab's menu asks for: its pin, closing one of its panes, or one of the agent's actions.
#[derive(Debug, Clone, PartialEq)]
enum TabPick {
    Pin(PinAct),
    ClosePane(PaneId),
    Agent(AgentPick),
}

/// The menu hung from a tab's row or tile, opened by a secondary press on it: Pin Tab or Unpin
/// Tab, Close Pane when it was opened from one pane of a split (`close`), then the agent's
/// actions when the tab has a pane.
#[component]
#[allow(non_snake_case)]
pub fn AgentTabMenu(
    tab: TabId,
    at: Rect,
    pin: PinAct,
    close: Option<PaneId>,
    desk: Desk,
    onclose: EventHandler<()>,
) -> Element {
    let menu = desk.hub.0.borrow().agent_menu(tab);
    let words = match pin {
        PinAct::Pin => Action::PinTab.menu_title(),
        PinAct::Unpin => Action::UnpinTab.menu_title(),
    };
    let mut items = vec![MenuItem::new(TabPick::Pin(pin), words)];
    if let Some(pane) = close {
        items.push(MenuItem::new(
            TabPick::ClosePane(pane),
            Action::ClosePane.menu_title(),
        ));
    }
    if let Some(menu) = menu {
        items.push(MenuItem::Separator);
        items.extend(
            items_of(&menu)
                .into_iter()
                .map(|item| item.map(&TabPick::Agent)),
        );
    }
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Rect(at),
            items,
            onpick: move |pick: TabPick| match pick {
                // A pinned tab opens again as a shell: it runs nothing of its own.
                TabPick::Pin(PinAct::Pin) => desk.act(WorkspaceIn::Tabs(pin_as_shell(tab))),
                TabPick::Pin(PinAct::Unpin) => desk.act(WorkspaceIn::Tabs(TabsIn::Unpin(tab))),
                TabPick::ClosePane(pane) => desk.act(WorkspaceIn::ClosePane(Some(pane))),
                TabPick::Agent(pick) => desk.agent_pick(tab, pick),
            },
            onclose,
        }
    }
}

/// A sheet's button.
fn press(label: &str, onclick: impl FnMut(Press) + 'static) -> Element {
    rsx! {
        Button { label: label.to_owned(), onclick: EventHandler::new(onclick) }
    }
}

/// The sheet over the window, when the hub has one.
#[component]
#[allow(non_snake_case)]
pub fn AgentSheets(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let view = desk.hub.0.borrow().agent_sheet();
    let Some(view) = view else {
        return rsx! {};
    };
    let close = {
        let desk = desk.clone();
        EventHandler::new(move |()| desk.close_agent_sheet())
    };
    match view {
        SheetView::Restore {
            title,
            put_back,
            bring_back,
            remove,
            note,
        } => {
            let (yes, no) = (desk.clone(), desk.clone());
            let nothing = put_back.is_empty() && bring_back.is_empty() && remove.is_empty();
            rsx! {
                Sheet { label: title.clone(), onclose: close,
                    SectionHeader { title }
                    if nothing {
                        Label { text: "Nothing has changed since this point", style: LabelStyle::Body }
                    }
                    for file in put_back {
                        Label { text: format!("Put back: {file}"), style: LabelStyle::Body }
                    }
                    for file in bring_back {
                        Label { text: format!("Bring back: {file}"), style: LabelStyle::Body }
                    }
                    for file in remove {
                        Label { text: format!("Remove, made since: {file}"), style: LabelStyle::Body }
                    }
                    Label { text: note, style: LabelStyle::Caption }
                    {press("Close", move |_| no.close_agent_sheet())}
                    {press("Restore…", move |_| yes.perform_restore())}
                }
            }
        }
        SheetView::Details {
            tab,
            title,
            lines,
            note,
        } => {
            let (merge, done) = (desk.clone(), desk.clone());
            rsx! {
                Sheet { label: title.clone(), onclose: close,
                    SectionHeader { title }
                    for line in lines {
                        Label { text: line, style: LabelStyle::Body }
                    }
                    Label { text: note, style: LabelStyle::Caption }
                    {press("Merge back (Squash)", move |_| {
                        merge.agent_pick(tab, AgentPick::MergeBack);
                        merge.close_agent_sheet();
                    })}
                    {press("Close", move |_| done.close_agent_sheet())}
                }
            }
        }
        SheetView::Leftovers { rows } => {
            let done = desk.clone();
            rsx! {
                Sheet { label: "Leftover branches".to_owned(), onclose: close,
                    SectionHeader { title: "Leftover branches".to_owned() }
                    for row in rows {
                        {leftover(&desk, row.branch, row.path)}
                    }
                    {press("Done", move |_| done.close_agent_sheet())}
                }
            }
        }
    }
}

/// One leftover: its branch and folder, and what can be done with it.
fn leftover(desk: &Desk, branch: String, path: String) -> Element {
    let (open, discard) = (desk.clone(), desk.clone());
    let (open_path, discard_path) = (path.clone(), path.clone());
    rsx! {
        Label { text: branch, style: LabelStyle::Body }
        Label { text: path, style: LabelStyle::Caption }
        {press("Open", move |_| open.open_leftover(open_path.clone()))}
        {press("Discard", move |_| discard.discard_leftover(discard_path.clone()))}
    }
}

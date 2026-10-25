//! The menu on a tab's row: its pin, and closing one pane of a split. The words are the actions'
//! own; this view draws them with quire's menu and sends the person's choice back.

use dioxus::prelude::*;
use ds::host::measure::Anchor;
use ds::prelude::*;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::{TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use crate::hub::Desk;

/// Whether a tab's menu pins it or lets it go: a row's pins, a pinned tile's unpins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinAct {
    Pin,
    Unpin,
}

/// What a tab's menu asks for: its pin, or closing one of its panes.
#[derive(Debug, Clone, PartialEq)]
enum TabPick {
    Pin(PinAct),
    ClosePane(PaneId),
}

/// The menu hung from a tab's row or tile, opened by a secondary press on it: Pin Tab or Unpin
/// Tab, and Close Pane when it was opened from one pane of a split (`close`).
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
    rsx! {
        Menu {
            placement: MenuPlacement::Context,
            anchor: Anchor::Rect(at),
            items,
            onpick: move |pick: TabPick| match pick {
                // A pinned tab opens again as a shell: it runs nothing of its own.
                TabPick::Pin(PinAct::Pin) => desk.act(WorkspaceIn::Tabs(TabsIn::pin_as_shell(tab))),
                TabPick::Pin(PinAct::Unpin) => desk.act(WorkspaceIn::Tabs(TabsIn::Unpin(tab))),
                TabPick::ClosePane(pane) => desk.act(WorkspaceIn::ClosePane(Some(pane))),
            },
            onclose,
        }
    }
}

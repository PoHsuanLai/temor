//! The console's bar (PLAN §7, v14 B2): over a row's terminal, Find and the row's "…" menu (the
//! one menu, as the row's right click opens it). Find opens its field in the bar itself; the
//! quick terminal is in no row and keeps the find bar on its corner.

use dioxus::prelude::*;
use ds::host::measure::use_rect;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::model::vocab::ids::PaneId;
use temor_panes::pane::keys::Action;

use super::row_menu::RowMenu;
use super::tool_button::ToolButton;
use crate::hub::{ConsoleRow, Desk};

/// The bar's two tools, for the terminal `pane` in `row`. `finding` is whether the find field is
/// open; `on_find` opens or closes it.
#[component]
#[allow(non_snake_case)]
pub fn ConsoleTools(
    pane: PaneId,
    row: ConsoleRow,
    finding: bool,
    desk: Desk,
    on_find: EventHandler<()>,
) -> Element {
    let more = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    rsx! {
        span { class: "temor-console-tools",
            span {
                class: "temor-page-tool",
                "data-pressed": finding.then_some("true"),
                ToolButton {
                    icon: Icon::Search,
                    size: ControlSize::Small,
                    label: Action::Find.label(),
                    tip: desk.hub.tooltip("Find in this tab", Action::Find),
                    onclick: move |()| on_find.call(()),
                }
            }
            span {
                class: "temor-page-tool",
                onmounted: move |event: MountedEvent| more.on_mounted(event),
                ToolButton {
                    icon: Icon::Ellipsis,
                    size: ControlSize::Small,
                    label: "More",
                    tip: "What can be done with this tab",
                    onclick: move |()| menu.set(Shown::Visible),
                }
            }
        }
        if let (Shown::Visible, Some(at)) = (menu(), more.rect()) {
            RowMenu {
                tab: row.tab,
                at,
                pinned: row.pinned,
                place: row.place.clone(),
                close: row.split.then_some(pane),
                desk: desk.clone(),
                onclose: move |()| menu.set(Shown::Hidden),
            }
        }
    }
}

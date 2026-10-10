//! One tab's row in the sidebar (PLAN §7): the companion's orb (idle until the companion exists),
//! the tab's title, the folder its pane is in, a dot when it has news, and a miniature when the
//! tab is split. Pressing the row brings the tab to the front; dragging it onto another row joins
//! the two tabs.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::{Availability, DropState, Emphasis, RowState};
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::components::content::label::LabelStyle;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::overlays::drag_ghost::DropLine;
use ds::host::measure::use_rect;
use ds::prelude::*;
use temor_core::presence::companion_presence;
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};

use super::dnd::{Click, Dnd, is_primary, point_of};
use super::miniature::Miniature;
use super::radar::AgentTabMenu;
use crate::hub::{Desk, DragKey, TabRow, Unread};

/// The row for one tab.
#[component]
#[allow(non_snake_case)]
pub fn TabRowView(row: TabRow, desk: Desk, dnd: Dnd) -> Element {
    let id = row.id;
    let rect = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    // The drag finds the row under the pointer from where the rows are.
    if let Some(at) = rect.rect() {
        dnd.rects.set(id, at);
    }
    {
        let rects = dnd.rects.clone();
        use_drop(move || rects.forget(id));
    }
    let drop = match (dnd.over)() {
        Some(over) if over == id => DropState::Target,
        _ => match dnd.drag.phase() {
            ds::motion::drag::DragPhase::Live {
                key: DragKey::Tab(held),
                ..
            } if held == id => DropState::Source,
            _ => DropState::Idle,
        },
    };
    let emphasis = match row.unread {
        Unread::Unread => Emphasis::Strong,
        Unread::Seen => Emphasis::Plain,
    };
    let content = rsx! {
        span { class: "temor-tab",
            CompanionOrb { presence: companion_presence(row.agent), size: OrbSize::Inline }
            span { class: "temor-tab-words",
                Label { text: row.title.clone(), style: LabelStyle::Body }
                if let Some(cwd) = row.cwd.clone() {
                    Label { text: cwd, style: LabelStyle::Caption }
                }
                if let Some(facts) = row.facts.clone() {
                    Label { text: facts, style: LabelStyle::Caption }
                }
                if let Some(note) = row.note.clone() {
                    Label { text: note, style: LabelStyle::Caption }
                }
            }
            if row.unread == Unread::Unread {
                Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
            }
        }
        if !row.tiles.is_empty() {
            Miniature { tiles: row.tiles.clone(), desk: desk.clone(), dnd: dnd.clone() }
        }
        if drop == DropState::Target {
            DropLine {}
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            AgentTabMenu {
                tab: id,
                at,
                desk: desk.clone(),
                onclose: move |()| menu.set(Shown::Hidden),
            }
        }
    };
    let click = dnd.click.clone();
    let selecting = desk.clone();
    rsx! {
        Row {
            title: row.title.clone(),
            content: Some(content),
            state: RowState {
                selection: row.selection,
                emphasis,
                availability: Availability::Enabled,
                drop,
            },
            onclick: move |press: Press| {
                // A secondary press opens the tab's agent menu; it does not select the tab.
                if matches!(press.button, PointerButton::Secondary) {
                    menu.set(Shown::Visible);
                    return;
                }
                if click.take() == Click::Counts {
                    selecting.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(id))));
                }
            },
            onpointerdown: move |event: PointerEvent| {
                if is_primary(&event) {
                    dnd.click.reset();
                    dnd.drag.down(DragKey::Tab(id), point_of(&event));
                }
            },
            common: Common {
                mounted: Some(EventHandler::new(move |event: MountedEvent| rect.on_mounted(event))),
                ..Common::default()
            },
        }
    }
}

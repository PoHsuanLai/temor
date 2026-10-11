//! One tab's row in the sidebar (PLAN §7): quire's `Row` with the tab's name and, under it, its
//! branch and state; trailing, the companion's orb (idle until the companion exists), a dot when
//! it has news, and a small glyph of the split when the tab has several panes. Pressing the row
//! brings the tab to the front; dragging it onto another row joins the two tabs.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::{Availability, DropState, Emphasis, RowState};
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::overlays::drag_ghost::DropLine;
use ds::components::overlays::tooltip::Tooltip;
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
    // The orb, the news dot and the miniature trail the row; the words are the row's own title
    // and detail, so their sizes are quire's (13 and 11.5).
    let trail = rsx! {
        CompanionOrb { presence: companion_presence(row.agent), size: OrbSize::Inline }
        if row.unread == Unread::Unread {
            Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
        }
        if !row.tiles.is_empty() {
            Miniature { tiles: row.tiles.clone(), desk: desk.clone(), dnd: dnd.clone() }
        }
    };
    let overlays = rsx! {
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
    let line = rsx! {
        div { class: "temor-tab-box",
            Row {
                title: shell_data(&row.title),
                detail: Some(detail_of(&row)),
                accessory: Accessory::Slot(trail),
                state: RowState {
                    selection: row.selection,
                    emphasis,
                    availability: Availability::Enabled,
                    drop,
                },
                onclick: move |press: Press| {
                    // A secondary press opens the tab's agent menu; it does not select the tab.
                    if matches!(press.button, PointerButton::Secondary) {
                        selecting.agent_menu_opened(id);
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
            {overlays}
        }
    };
    // What the agent in the tab is doing is said on hover, where the row has no line for it.
    match row.note.clone() {
        Some(note) => rsx! {
            Tooltip { text: note, {line} }
        },
        None => line,
    }
}

/// Shell data (a folder, a program, a branch). quire's `Code` tone is an inline code chip, not
/// plain fixed pitch, so it is set in the row's own face until quire has a plain fixed-pitch run.
fn shell_data(text: &str) -> TextLine {
    TextLine::Plain(text.to_owned())
}

/// The second line: the branch (shell data), then what the tab is doing.
fn detail_of(row: &TabRow) -> TextLine {
    let state = row.state.words();
    match &row.branch {
        Some(branch) => TextLine::Plain(format!("{branch} \u{b7} {state}")),
        None => TextLine::Plain(state),
    }
}

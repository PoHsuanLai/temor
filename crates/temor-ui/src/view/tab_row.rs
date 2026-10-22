//! One tab's row in the sidebar (PLAN §7): quire's `Row` on one line, as mailo's and Arc's are,
//! led by a terminal glyph, or by the companion's orb while an agent works or waits, with the
//! tab's name and, trailing, a dot when it has news. Its branch and state are said on hover. A tab
//! with several panes is Arc's split row instead: a segment per pane (`split_row`). Pressing the
//! row brings the tab to the front; dragging it onto another row joins the two tabs.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::{Availability, CompanionPresence, DropState, Emphasis, RowState};
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::components::content::label::{LabelRole, LabelStyle};
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::controls::press::Propagation;
use ds::components::overlays::drag_ghost::DropLine;
use ds::prelude::IconView;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::presence::companion_presence;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::WorkspaceIn;

use super::dnd::Dnd;
use super::radar::{AgentTabMenu, PinAct};
use super::row_tip::RowTip;
use super::split_row::SplitSegments;
use super::tab_glyph::prompt_glyph;
use super::tab_wiring::{drag_press, tab_menu_trigger, use_drag_target};
use super::tool_button::ToolButton;
use crate::hub::{Desk, DragKey, TabRow, TabTone, Unread, select_tab};

/// The row for one tab.
#[component]
#[allow(non_snake_case)]
pub fn TabRowView(row: TabRow, desk: Desk, dnd: Dnd) -> Element {
    let id = row.id;
    let mut menu = use_signal(|| Shown::Hidden);
    // The pane whose segment was right-clicked: the menu offers to close it.
    let mut menu_pane = use_signal(|| None::<PaneId>);
    // The drag finds the row under the pointer from where the rows are.
    let rect = use_drag_target(id, &dnd.rects);
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
    // The prompt glyph leads the row, or the companion's orb while an agent works or waits (a
    // press on it is a press on the row); the news dot trails it. The words
    // are the row's own title and detail, so their sizes are quire's (13 and 11.5).
    // A tab whose last command succeeded or failed wears that colour on its glyph.
    let leading = match (companion_presence(row.agent), row.tone) {
        (CompanionPresence::Idle, TabTone::Plain) => {
            RowLeading::Source(prompt_glyph(IconSize::Small))
        }
        (CompanionPresence::Idle, tone) => RowLeading::Element(rsx! {
            span { class: "temor-tab-glyph", "data-tone": tone.slug(),
                IconView { source: prompt_glyph(IconSize::Small), size: IconSize::Tile }
            }
        }),
        (presence, _) => RowLeading::Element(rsx! {
            CompanionOrb { presence, size: OrbSize::Inline }
        }),
    };
    let closing = desk.clone();
    // While a command runs, how long it has, at the row's end: the row has no line for the state.
    let elapsed = row.state.elapsed();
    let trail = rsx! {
        if let Some(elapsed) = elapsed {
            span { class: "temor-tab-time ds-mono",
                Label { text: elapsed, role: LabelRole::Tertiary, style: LabelStyle::Footnote }
            }
        }
        if row.unread == Unread::Unread {
            Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
        }
        span {
            class: "temor-tab-close",
            "data-selected": (row.selection == Selection::Selected).to_string(),
            ToolButton {
                icon: Icon::X,
                size: ControlSize::Mini,
                propagation: Propagation::Stop,
                label: "Close tab",
                tip: "Close this tab",
                onclick: move |()| closing.act(WorkspaceIn::CloseTab(id)),
            }
        }
    };
    // A split is its segments, each with its own glyph, so the row leads with none.
    let (leading, content) = match row.tiles.is_empty() {
        true => (leading, None),
        false => (
            RowLeading::None,
            Some(rsx! {
                SplitSegments {
                    tab: id,
                    tiles: row.tiles.clone(),
                    desk: desk.clone(),
                    dnd: dnd.clone(),
                    onmenu: move |pane: PaneId| {
                        menu_pane.set(Some(pane));
                        menu.set(Shown::Visible);
                    },
                }
            }),
        ),
    };
    let overlays = rsx! {
        if drop == DropState::Target {
            DropLine {}
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            AgentTabMenu {
                tab: id,
                at,
                pin: PinAct::Pin,
                close: menu_pane(),
                desk: desk.clone(),
                onclose: move |()| {
                    menu.set(Shown::Hidden);
                    menu_pane.set(None);
                },
            }
        }
    };
    let click = dnd.click.clone();
    let selecting = desk.clone();
    let opening = tab_menu_trigger(&desk, id, move || menu.set(Shown::Visible));
    let dragging = drag_press(&dnd, DragKey::Tab(id));
    let tip = hover_text(&row);
    rsx! {
        div { class: "temor-tab-box", oncontextmenu: opening,
            // The branch, the state and what an agent is doing are said on hover: the row is
            // one line.
            RowTip { name: format!("tab{}", id.0), text: tip,
            Row {
                title: row.title.as_str(),
                content,
                leading,
                accessory: Accessory::Slot(trail),
                state: RowState {
                    selection: row.selection,
                    emphasis,
                    availability: Availability::Enabled,
                    drop,
                },
                onclick: move |press: Press| {
                    // A secondary press is the tab's menu (`oncontextmenu` above); it does not
                    // select the tab.
                    if matches!(press.button, PointerButton::Secondary) {
                        return;
                    }
                    click.counted(|| {
                        selecting.act(select_tab(id));
                    });
                },
                onpointerdown: dragging,
                common: Common {
                    mounted: Some(EventHandler::new(move |event: MountedEvent| rect.on_mounted(event))),
                    ..Common::default()
                },
            }
            }
            {overlays}
        }
    }
}

/// What hover says: the branch and the state, then what the agent in the tab is doing.
fn hover_text(row: &TabRow) -> String {
    hover_words(
        row.branch.as_deref(),
        row.state.words(),
        row.note.as_deref(),
    )
}

/// The hover lines for a tab's `branch`, `state` words and agent `note`.
fn hover_words(branch: Option<&str>, state: String, note: Option<&str>) -> String {
    let head = match branch {
        Some(branch) => format!("{branch} \u{b7} {state}"),
        None => state,
    };
    match note {
        Some(note) => format!("{head}\n{note}"),
        None => head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_says_the_branch_and_state_then_the_agents_note() {
        let running = "running 2m";
        let table = [
            ("state only", None, "idle", None, "idle"),
            (
                "branch",
                Some("main"),
                running,
                None,
                "main \u{b7} running 2m",
            ),
            (
                "note under state",
                None,
                "idle",
                Some("On branch agent/fix"),
                "idle\nOn branch agent/fix",
            ),
            (
                "all three",
                Some("agent/fix"),
                running,
                Some("src/lib.rs is also changed in Docs (reads text only)"),
                "agent/fix \u{b7} running 2m\nsrc/lib.rs is also changed in Docs (reads text only)",
            ),
        ];
        for (name, branch, state, note, want) in table {
            assert_eq!(hover_words(branch, state.to_owned(), note), want, "{name}");
        }
    }
}

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
use ds::components::content::text_runs::{RunTone, TextLine, TextRun};
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::overlays::drag_ghost::DropLine;
use ds::host::measure::use_rect;
use ds::prelude::IconView;
use ds::prelude::*;
use temor_core::presence::companion_presence;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};

use super::dnd::{Click, Dnd, is_primary, point_of};
use super::radar::{AgentTabMenu, PinAct};
use super::row_tip::RowTip;
use super::split_row::SplitSegments;
use super::tab_glyph::prompt_glyph;
use crate::hub::{Desk, DragKey, TabRow, TabTone, Unread};

/// The row for one tab.
#[component]
#[allow(non_snake_case)]
pub fn TabRowView(row: TabRow, desk: Desk, dnd: Dnd) -> Element {
    let id = row.id;
    let rect = use_rect();
    let mut menu = use_signal(|| Shown::Hidden);
    // The pane whose segment was right-clicked: the menu offers to close it.
    let mut menu_pane = use_signal(|| None::<PaneId>);
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
    };
    // A split is its segments, each with its own glyph, so the row leads with none.
    let (leading, content) = match row.tiles.is_empty() {
        true => (leading, None),
        false => (
            RowLeading::None,
            Some(rsx! {
                SplitSegments {
                    tiles: row.tiles.clone(),
                    desk: desk.clone(),
                    dnd: dnd.clone(),
                    onmenu: {
                        let desk = desk.clone();
                        move |pane: PaneId| {
                            desk.agent_menu_opened(id);
                            menu_pane.set(Some(pane));
                            menu.set(Shown::Visible);
                        }
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
    let tip = hover_text(&row);
    rsx! {
        div { class: "temor-tab-box",
            // The branch, the state and what an agent is doing are said on hover: the row is
            // one line.
            RowTip { name: format!("tab{}", id.0), text: tip,
            Row {
                title: shell_data(&row.title),
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
            }
            {overlays}
        }
    }
}

/// Shell data (a folder, a program, a branch): fixed pitch on the row's own ground and ink.
fn shell_data(text: &str) -> TextLine {
    TextLine::Runs(vec![TextRun::new(text, RunTone::Mono)])
}

/// What hover says: the branch and the state, then what the agent in the tab is doing.
fn hover_text(row: &TabRow) -> String {
    let state = row.state.words();
    let head = match &row.branch {
        Some(branch) => format!("{branch} \u{b7} {state}"),
        None => state,
    };
    match &row.note {
        Some(note) => format!("{head}\n{note}"),
        None => head,
    }
}

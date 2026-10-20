//! The quick terminal over the window (PLAN §7): a small terminal in front of the tab, with its
//! chord to hide it and "Move to Space" to keep it as a tab. Hiding unmounts the view only; the
//! shell keeps running in the hub, and the same chord brings the same shell back.

use std::time::Instant;

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::prelude::*;
use ds::style::space::list::SpaceId;
use temor_core::bar::SpaceEntry;
use temor_core::vocab::ids::PaneId;

use super::pane_view::PaneView;
use crate::hub::Desk;
use temor_pane::keys::Action;

/// The quick terminal's overlay. `spaces` are the places it can be moved to; with none, it is
/// kept as a tab of this window.
#[component]
#[allow(non_snake_case)]
pub fn QuickView(pane: PaneId, spaces: Vec<SpaceEntry>, desk: Desk, started: Instant) -> Element {
    let places = match spaces.is_empty() {
        // TODO(spaces): the window has one workspace until Spaces reach it; the id is not read.
        true => vec![(SpaceId(0), "Keep as a tab".to_owned())],
        false => spaces
            .iter()
            .map(|space| (space.id, format!("Move to {}", space.name)))
            .collect(),
    };
    let hide = desk.clone();
    rsx! {
        div { class: "temor-quick", "aria-label": "Quick terminal",
            div { class: "temor-quick-head",
                for (space, label) in places {
                    Button {
                        key: "{space.0}",
                        label,
                        title: Some("Keep this terminal, still running, as a tab".to_owned()),
                        onclick: {
                            let desk = desk.clone();
                            move |_: Press| desk.quick_move(space)
                        },
                    }
                }
                Button {
                    label: "Hide",
                    title: Some(desk.hub.tooltip("Hide the quick terminal; it keeps running", Action::ToggleQuick)),
                    onclick: move |_: Press| hide.quick_toggle(),
                }
            }
            div { class: "temor-quick-body",
                PaneView { pane, desk: desk.clone(), started }
            }
        }
    }
}

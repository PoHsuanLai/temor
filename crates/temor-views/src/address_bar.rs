//! The top of the sidebar under the Space's name (PLAN §7, v14 B1): Back and Forward for the row
//! in front, and its address, the folders down to what it shows (the page on its stage, or its
//! terminal's folder) on quire's path bar. A folder in it opens that folder's page; the middle of
//! a long path folds into "…", a menu of the folders folded.

use dioxus::prelude::*;
use ds::components::chrome::path_control::model::PathSegment;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_panes::pane::keys::Action;
use temor_schema::url::TemorUrl;

use super::tool_button::ToolButton;
use temor_hub::{Address, Desk, temor_page_name, use_hub};

/// Back and Forward through the pages the row in front has shown.
#[component]
#[allow(non_snake_case)]
pub fn WalkButtons(desk: Desk) -> Element {
    let (view, back_tip, forward_tip) = use_hub(&desk, (), |hub, ()| {
        let view = hub
            .front()
            .map(|(tab, ..)| hub.stage_view(tab))
            .unwrap_or_default();
        let tip = |action: Action| hub.tooltip(action.label(), action);
        (view, tip(Action::Back), tip(Action::Forward))
    })();
    let can = |yes: bool| match yes {
        true => Availability::Enabled,
        false => Availability::Disabled,
    };
    let (back, forward) = (desk.clone(), desk.clone());
    rsx! {
        span { class: "temor-walk",
            ToolButton {
                icon: Icon::ChevronLeft,
                size: ControlSize::Small,
                label: Action::Back.label(),
                tip: back_tip,
                availability: can(view.can_go_back),
                onclick: move |()| back.go_back(),
            }
            ToolButton {
                icon: Icon::ChevronRight,
                size: ControlSize::Small,
                label: Action::Forward.label(),
                tip: forward_tip,
                availability: can(view.can_go_forward),
                onclick: move |()| forward.go_forward(),
            }
        }
    }
}

/// The address of the row in front.
#[component]
#[allow(non_snake_case)]
pub fn AddressBar(desk: Desk) -> Element {
    let address = use_hub(&desk, (), |hub, ()| hub.address())();
    let (segments, paths): (Vec<PathSegment>, Vec<Option<String>>) = match &address {
        Address::None => return rsx! {},
        Address::Page(page) => {
            let icon = match page {
                TemorUrl::Downloads => Icon::Download,
                _ => Icon::Clock,
            };
            (
                vec![PathSegment::new(temor_page_name(page), icon)],
                vec![None],
            )
        }
        Address::Path { crumbs, file } => {
            let last = crumbs.len() - 1;
            crumbs
                .iter()
                .enumerate()
                .map(|(at, crumb)| {
                    let icon = match (at, crumb.name.as_str()) {
                        (_, _) if at == last && *file => Icon::File,
                        (0, "~") => Icon::Home,
                        (0, _) => Icon::DriveInternal,
                        _ => Icon::Folder,
                    };
                    let open = !(at == last && *file);
                    (
                        PathSegment::new(crumb.name.clone(), icon),
                        open.then(|| crumb.path.clone()),
                    )
                })
                .unzip()
        }
    };
    let opening = desk.clone();
    rsx! {
        div { class: "temor-address",
            PathControl {
                label: "Address",
                segments,
                room: ADDRESS_ROOM,
                onpick: move |at: usize| {
                    if let Some(Some(path)) = paths.get(at) {
                        opening.open_folder(path);
                    }
                },
            }
        }
    }
}

/// How many places the address has before its middle folds: the sidebar is narrow.
const ADDRESS_ROOM: usize = 4;

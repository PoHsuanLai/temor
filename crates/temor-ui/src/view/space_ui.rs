//! Spaces in the window (PLAN §7), on quire's Spaces kit. The kit owns the list as a signal and
//! drives switching, renaming, colouring and deleting; the hub keeps a workspace for each Space
//! and follows the kit through the four callbacks `use_window_spaces` hands it.

use dioxus::prelude::*;
use ds::base::vocab::Availability;
use ds::components::app::spaces::{
    NewSpace, SpaceHead, SpaceId, SpaceMenu, SpacesFoot, SwitchChord, Switched, use_spaces,
};
use ds::components::menus::item::item::MenuItem;
use ds::prelude::{Point, Px};
use temor_client::model::space::{Place, Project, TemorSpaces};

use super::shelf::ShelfButton;
use super::space_handle::SpacesOf;
use crate::hub::{Desk, HubRef};

/// The hub's Spaces as the kit's handle for the window. `ui` and `notice` are the window's
/// redraw signal and message bar.
pub fn use_window_spaces(
    hub: &HubRef,
    ui: Signal<u32>,
    notice: Signal<Option<String>>,
) -> SpacesOf {
    let tell = move |texts: Vec<String>| {
        let mut notice = notice;
        if let Some(text) = texts.into_iter().last() {
            notice.set(Some(text));
        }
        let mut ui = ui;
        ui += 1;
    };
    let (booted, kept, here, arrived) = (hub.clone(), hub.clone(), hub.clone(), hub.clone());
    use_spaces(
        move || booted.boot_spaces(),
        move |all: &TemorSpaces| tell(kept.spaces_changed(all)),
        move || here.here(),
        move |switched: Switched<Place>| tell(arrived.arrive(switched.to)),
    )
}

/// The only thing temor adds to a Space's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpaceAct {
    /// Bring the tab in front into this Space.
    MoveTabHere,
}

/// The head of the sidebar, as mailo's is: the name of the Space on screen, one Space or several,
/// so it says where you are; Downloads stands at the row's right end.
#[component]
#[allow(non_snake_case)]
pub fn SpaceHeader(desk: Desk) -> Element {
    rsx! {
        div { class: "temor-head",
            div { class: "temor-head-title",
                SpaceHead { handle: desk.spaces }
            }
            ShelfButton { desk: desk.clone() }
        }
    }
}

/// A right click anywhere in the sidebar is the current Space's menu, as mailo's is, unless what
/// it landed on has a menu of its own (a tab, a Space's dot, a download), which takes the click
/// (its `prevent_default`) before it gets here.
pub fn space_menu_on(desk: &Desk) -> impl FnMut(MouseEvent) + 'static {
    let handle = desk.spaces;
    move |event: MouseEvent| {
        if !event.default_action_enabled() {
            return;
        }
        event.prevent_default();
        let at = event.client_coordinates();
        handle.open_menu(
            handle.current(),
            Point {
                x: Px(at.x as f32),
                y: Px(at.y as f32),
            },
        );
    }
}

/// The foot of the sidebar: a dot for each Space once there are several (quire's `SpacesFoot`
/// draws none for one) between its two ends, `trailing` the right one (the foot draws no `+`:
/// New Space is in the Space's menu). The Space switch has no chord of its own here: the desktop
/// owns Ctrl+digit.
#[component]
#[allow(non_snake_case)]
pub fn SpaceFoot(desk: Desk, trailing: Element) -> Element {
    let new_payload = use_callback(|()| Project::default());
    let handle = desk.spaces;
    let many = handle.spaces().read().list().len() > 1;
    // Each end is a hold as wide as the other, so the dots stand at the foot's centre.
    let leading = rsx! { div { class: "temor-foot-lead" } };
    if many {
        return rsx! {
            SpacesFoot {
                handle,
                new_payload,
                chord: SwitchChord::Unbound,
                new_space: NewSpace::Hidden,
                leading,
                trailing,
            }
        };
    }
    rsx! {
        div { class: "temor-foot-one",
            {leading}
            {trailing}
        }
    }
}

/// The Space's menu (rename, colour, appearance, new, delete) and "Move Tab Here".
#[component]
#[allow(non_snake_case)]
pub fn SpaceMenus(desk: Desk) -> Element {
    let handle = desk.spaces;
    let new_payload = use_callback(|()| Project::default());
    // Moving a tab into the Space it is already in means nothing.
    let availability = match handle.open().map(|open| open.space) == Some(handle.current()) {
        true => Availability::Disabled,
        false => Availability::Enabled,
    };
    let extra =
        vec![MenuItem::new(SpaceAct::MoveTabHere, "Move Tab Here").with_availability(availability)];
    rsx! {
        SpaceMenu::<Project, Place, SpaceAct> {
            handle,
            new_payload,
            extra,
            on_extra: move |(space, act): (SpaceId, SpaceAct)| match act {
                SpaceAct::MoveTabHere => desk.move_front_tab_to(space),
            },
            kept: "Everything on disk".to_owned(),
        }
    }
}

impl Desk {
    /// Moves the tab in front to `space`.
    pub fn move_front_tab_to(&self, space: SpaceId) {
        if let Some((tab, _, _)) = self.hub.front() {
            for text in self.hub.move_tab_to_space(tab, space) {
                self.say(text);
            }
        }
        self.bump();
    }
}

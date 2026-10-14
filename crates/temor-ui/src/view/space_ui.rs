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
use temor_core::space::{Place, Project, TemorSpaces};

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

/// The head of the sidebar, as mailo's is: the name of the Space on screen, with its menu on a
/// right click, and Downloads at the row's right end. With one Space there is no name: Spaces
/// show only once a second one exists, and the row holds Downloads alone.
#[component]
#[allow(non_snake_case)]
pub fn SpaceHeader(desk: Desk) -> Element {
    let many = desk.spaces.spaces().read().list().len() > 1;
    rsx! {
        div { class: "temor-head",
            div { class: "temor-head-title",
                if many {
                    SpaceHead { handle: desk.spaces }
                }
            }
            ShelfButton { desk: desk.clone() }
        }
    }
}

/// The foot of the sidebar: a dot for each Space, and its `trailing` (the chevron, whose menu
/// holds New Space: the foot draws no `+`). The Space switch has no chord of its own here: the
/// desktop owns Ctrl+digit. With one Space there are no dots, and a right click on the foot is
/// that Space's menu, which the head's name holds once there are two.
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
        div {
            class: "temor-foot-one",
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                let at = event.client_coordinates();
                handle.open_menu(handle.current(), Point { x: Px(at.x as f32), y: Px(at.y as f32) });
            },
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
    /// Switches to the Space with number `number` (1 to 9), as the Space chords count them.
    pub fn select_space(&self, number: u8) {
        if let Some(index) = usize::from(number).checked_sub(1) {
            let _ = self.spaces.switch_index(index);
        }
    }

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

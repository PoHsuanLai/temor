//! The chevron at the end of the sidebar's foot and its menu, as mailo's and Dia's are (quire's
//! pull-down in its chevron face): New Tab, Reopen Closed Tab, New Space and Hide Sidebar. The
//! foot draws no `+` of its own; New Space is here, and opens the new Space's name at the chevron.

use dioxus::prelude::*;
use ds::components::app::spaces::Showing;
use ds::components::menus::item::item::MenuItem;
use ds::components::menus::pull_down_button::{PullDownButton, PullDownFace};
use ds::host::measure::{MountedRef, client_rect};
use ds::prelude::*;
use ds_core::vocab::Shortcut;
use temor_core::space::Project;
use temor_pane::keys::Action;

use super::menu_key::with_key_opt;
use crate::hub::{Desk, Origin};

/// What the menu's rows ask for: an action the keys have too, or a new Space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePick {
    Do(Action),
    NewSpace,
}

/// The menu's rows, in order.
/// Rows whose action has a chord show it, as `shortcut_of` gives it.
pub fn side_items(shortcut_of: impl Fn(Action) -> Option<Shortcut>) -> Vec<MenuItem<SidePick>> {
    let keyed = |action: Action| {
        with_key_opt(
            MenuItem::new(SidePick::Do(action), action.menu_title()),
            shortcut_of(action),
        )
    };
    vec![
        keyed(Action::NewTab),
        keyed(Action::ReopenTab),
        MenuItem::Separator,
        MenuItem::new(SidePick::NewSpace, "New Space"),
        MenuItem::Separator,
        keyed(Action::ToggleSidebar),
    ]
}

/// The chevron and its menu.
#[component]
#[allow(non_snake_case)]
pub fn SideMenu(desk: Desk) -> Element {
    let mut button = use_signal(|| None::<MountedRef>);
    let handle = desk.spaces;
    rsx! {
        PullDownButton::<SidePick> {
            face: PullDownFace::Chevron,
            items: side_items(|action| desk.hub.shortcut_of(action)),
            common: Common {
                aria_label: Some("Sidebar Menu".to_owned()),
                mounted: Some(EventHandler::new(move |event: MountedEvent| {
                    button.set(Some(MountedRef(event.data())));
                })),
                ..Common::default()
            },
            onpick: move |pick: SidePick| match pick {
                SidePick::Do(action) => desk.run_action(action, Origin::Menu),
                SidePick::NewSpace => {
                    if handle.editing().is_some() {
                        return;
                    }
                    let made = handle.add(Project::default());
                    // The name field opens at the chevron, read once the menu has gone.
                    let at = button();
                    spawn(async move {
                        let point = match at {
                            Some(at) => client_rect(&at.0)
                                .await
                                .map(|rect| rect.origin)
                                .unwrap_or_default(),
                            None => Point::default(),
                        };
                        handle.show(made, point, Showing::Rename);
                    });
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ds_core::vocab::ShortcutKey;

    use super::*;

    /// The rows with their key equivalents, by title.
    fn keyed(shortcut_of: impl Fn(Action) -> Option<Shortcut>) -> Vec<(String, Option<Shortcut>)> {
        side_items(shortcut_of)
            .into_iter()
            .filter_map(|item| match item {
                MenuItem::Item { title, key, .. } => Some((title, key)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rows_with_an_action_show_its_chord_and_the_others_show_none() {
        let t = Shortcut(vec![ShortcutKey::Char('t')]);
        let rows = keyed(|action| (action == Action::NewTab).then(|| t.clone()));
        assert_eq!(
            rows,
            [
                ("New Tab".to_owned(), Some(t)),
                ("Reopen Closed Tab".to_owned(), None),
                ("New Space".to_owned(), None),
                ("Hide Sidebar".to_owned(), None),
            ]
        );
    }
}

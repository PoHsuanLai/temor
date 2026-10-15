//! Where a new tab and a split start, by the "New tabs start in" setting.

use temor_core::split::Direction;
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_pane::settings::StartIn;

use super::support::Rig;

/// The folder the workspace holds for `pane`.
fn folder(rig: &Rig, pane: PaneId) -> Option<String> {
    rig.hub
        .ws
        .cwds
        .get(&pane)
        .map(|cwd| cwd.as_str().to_owned())
}

#[test]
fn a_new_tab_and_a_split_start_where_the_setting_says() {
    let Some(mut rig) = Rig::new("start-in") else {
        return;
    };
    let here = "/tmp/start-in-here";
    rig.hub.act(WorkspaceIn::NewTab {
        cwd: Some(Cwd(here.to_owned())),
    });
    let origin = rig.front();
    // (setting, the folder a new tab is given, the folder a split is given)
    #[rustfmt::skip]
    let rows: &[(StartIn, Option<&str>, Option<&str>)] = &[
        (StartIn::Usual, None, Some(here)),
        (StartIn::Current, Some(here), Some(here)),
        (StartIn::Home, None, None),
    ];
    for (setting, tab, split) in rows {
        rig.hub.launch.settings.start_in = *setting;
        rig.hub.act(WorkspaceIn::Focus(origin));
        rig.hub.act(WorkspaceIn::NewTab { cwd: None });
        let new_tab = rig.front();
        assert_ne!(new_tab, origin, "{setting:?}");
        assert_eq!(folder(&rig, new_tab).as_deref(), *tab, "tab, {setting:?}");

        rig.hub.act(WorkspaceIn::Focus(origin));
        rig.hub.act(WorkspaceIn::Split {
            towards: Direction::Right,
        });
        let new_split = rig.front();
        assert_ne!(new_split, origin, "{setting:?}");
        assert_eq!(
            folder(&rig, new_split).as_deref(),
            *split,
            "split, {setting:?}"
        );
    }
}

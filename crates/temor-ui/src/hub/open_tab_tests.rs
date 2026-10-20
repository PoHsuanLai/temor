//! A tab opened for a request is found by what it added to the workspace, never by who has the
//! keyboard afterwards: the bar can be open over the quick terminal.

use temor_core::workspace::WorkspaceIn;

use super::model::Hub;
use super::tests::launch;
use crate::view::launch::LauncherChoice;

#[test]
fn the_pane_of_a_new_tab_is_the_tabs_even_when_the_quick_terminal_has_the_keyboard() {
    let Some(setsid) = temor_pty::find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin"))
    else {
        if std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1") {
            panic!("setsid is required (TEMOR_REQUIRE_TOOLS=1)");
        }
        eprintln!("SKIPPED: needs util-linux setsid");
        return;
    };
    let mut launch = launch();
    launch.launcher = LauncherChoice::Setsid(setsid);
    let mut hub = Hub::new(launch);
    hub.act(WorkspaceIn::NewTab { cwd: None });
    hub.quick_toggle();
    let quick = hub.quick_shown().expect("the quick terminal is shown");
    assert_eq!(hub.keyboard_pane(), Some(quick));

    let opened = hub.open_tab(None).expect("a tab opened");

    assert_ne!(opened, quick);
    assert!(hub.ws.tab_of(opened).is_some(), "the pane is a tab's");
    assert_eq!(
        hub.keyboard_pane(),
        Some(quick),
        "the keyboard stays with the quick terminal"
    );
}

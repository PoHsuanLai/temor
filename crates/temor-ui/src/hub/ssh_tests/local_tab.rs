//! A tab on this computer, asked for from a host pane through the bar.

use super::support::Rig;
use crate::hub::bar_local::LOCAL_TAB_TITLE;
use crate::hub::model::Hub;

/// The titles the bar lists for `query`.
fn titles(hub: &mut Hub, query: &str) -> Vec<String> {
    hub.bar_typed(query.to_owned());
    hub.bar_data()
        .map(|bar| bar.rows.into_iter().map(|row| row.title).collect())
        .unwrap_or_default()
}

#[test]
fn the_bar_offers_a_tab_on_this_computer_only_in_a_host_pane_and_it_opens_one() {
    let Some(mut rig) = Rig::new("local-tab") else {
        return;
    };
    let query = "on this computer";
    rig.hub.bar_toggle();
    assert!(!titles(&mut rig.hub, query).contains(&LOCAL_TAB_TITLE.to_owned()));
    rig.hub.bar_close();

    rig.hub.open_host_named("fake");
    let host = rig.front();
    assert_eq!(rig.hub.host_of(host), Some("fake"));
    rig.hub.bar_toggle();
    assert!(titles(&mut rig.hub, query).contains(&LOCAL_TAB_TITLE.to_owned()));
    let row = rig
        .hub
        .bar_data()
        .and_then(|bar| {
            bar.rows
                .into_iter()
                .find(|row| row.title == LOCAL_TAB_TITLE)
        })
        .expect("the row");
    let tabs = rig.hub.book.ws.tabs.place.tabs.len();
    rig.hub.bar_pick(&row.key);

    let local = rig.front();
    assert_ne!(local, host);
    assert_eq!(rig.hub.host_of(local), None, "a tab on this computer");
    assert_eq!(rig.hub.book.ws.tabs.place.tabs.len(), tabs + 1);
    // A plain new tab from the host pane still goes to the host.
    rig.hub
        .act(temor_client::model::workspace::WorkspaceIn::Focus(host));
    rig.hub
        .act(temor_client::model::workspace::WorkspaceIn::NewTab { cwd: None });
    assert_eq!(rig.hub.host_of(rig.front()), Some("fake"));
}

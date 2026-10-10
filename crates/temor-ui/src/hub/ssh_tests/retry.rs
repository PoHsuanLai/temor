//! Retry on a connection temor gave up on: the `ssh` that runs in the pane asks for a password,
//! the person types it, and the pane carries on (the stand-in `ssh` asks, and accepts one word).

use temor_core::remote::Grant;
use temor_ssh::HostAlias;
use temor_wire::link::{GoneReason, LinkState};

use super::support::Rig;
use crate::hub::ssh_view::Stage;

/// Types `text` into the pane.
fn type_into(rig: &Rig, pane: temor_core::vocab::ids::PaneId, text: &str) {
    if let Some(slot) = rig.hub.slots.get(&pane)
        && let Some(session) = slot.borrow().session.as_ref()
    {
        session.pane().send(text.as_bytes());
    }
}

#[test]
fn retry_asks_for_the_password_in_the_pane_and_carries_on_once_it_is_given() {
    let Some(mut rig) = Rig::new("retry") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    let dir = rig.dir.clone();
    rig.pump("the first session", |_| dir.join("session-1").exists());
    rig.put("up", "");
    rig.pump("the connection", |hub| {
        hub.host_view(pane).is_some_and(|v| v.stage == Stage::Up)
    });

    // The connection is lost for good, and the shared one is gone with it.
    let host = HostAlias::new("fake").expect("a name");
    std::fs::remove_file(rig.dir.join("up")).expect("the connection goes");
    rig.hub.remote.lines.entry(host).or_default().link = LinkState::Gone {
        reason: GoneReason::GaveUp,
    };

    rig.hub.remote_retry(pane);
    assert_eq!(rig.stage(pane), Stage::Connecting);
    rig.pump("the password prompt", |hub| {
        screen_has(hub, pane, "Password:")
    });
    assert!(
        rig.got("connect-ok").is_none(),
        "nothing connects before the password is typed"
    );

    // The right word makes the connection, and the pane is a session on the host again.
    type_into(&rig, pane, "hunter2\n");
    rig.pump("the connection to be made", |hub| {
        hub.host_view(pane).is_some_and(|v| v.stage == Stage::Up)
    });
    assert_eq!(rig.got("connect-ok").as_deref(), Some("hunter2\n"));
    rig.pump("the session on the host", |_| {
        dir.join("session-2").exists()
    });
}

fn screen_has(
    hub: &crate::hub::model::Hub,
    pane: temor_core::vocab::ids::PaneId,
    text: &str,
) -> bool {
    hub.slots
        .get(&pane)
        .and_then(|slot| slot.borrow().session.as_ref().map(|s| s.screen_text()))
        .is_some_and(|screen| screen.contains(text))
}

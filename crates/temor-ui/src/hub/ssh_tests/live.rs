//! A setting changed while panes are open: the ones open follow it.

use temor_client::model::remote::Grant;
use temor_panes::pane::settings::NotifyAfter;

use super::support::Rig;
use crate::settings::TerminalDoc;

#[test]
fn an_open_pane_follows_a_changed_notice_threshold() {
    let Some(mut rig) = Rig::new("notify-live") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    let after = |rig: &Rig| {
        rig.hub
            .slots
            .get(&pane)
            .and_then(|slot| slot.borrow().session.as_ref().map(|s| s.notify_after()))
    };
    let dir = rig.dir.clone();
    rig.pump("the pane's session", |hub| {
        dir.join("session-1").exists()
            && hub
                .slots
                .get(&pane)
                .is_some_and(|slot| slot.borrow().session.is_some())
    });
    assert_eq!(after(&rig), Some(NotifyAfter::DEFAULT));

    // (what the file says, what the open pane then holds)
    let rows = [
        (NotifyAfter::Seconds(45), NotifyAfter::Seconds(45)),
        (NotifyAfter::Off, NotifyAfter::Off),
    ];
    for (set, want) in rows {
        let mut doc = TerminalDoc::default();
        doc.notify.after = set;
        rig.hub.apply_settings(&doc);
        assert_eq!(after(&rig), Some(want));
    }
}

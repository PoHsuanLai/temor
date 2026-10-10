//! Editing a host's file locally and forwarding its ports, through the stand-in `ssh` and `scp`
//! (the stand-in `ssh` runs the file calls on this computer, so the "host" is a scratch folder).

use temor_core::host::edit::Resolution;
use temor_core::remote::Grant;
use temor_ssh::Port;

use super::support::Rig;
use crate::edge::ssh_work::EditId;
use crate::hub::ssh_view::{EditStatus, Stage};

/// A rig whose first host pane is connected.
fn connected(name: &str) -> Option<(Rig, temor_core::vocab::ids::PaneId)> {
    let mut rig = Rig::new(name)?;
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
    Some((rig, pane))
}

/// Waits until the host's file reads `text`.
fn until_remote(rig: &mut Rig, path: &std::path::Path, text: &str) {
    let path = path.to_path_buf();
    let want = text.to_owned();
    rig.pump("the host's file to change", move |_| {
        std::fs::read_to_string(&path).is_ok_and(|now| now == want)
    });
}

#[test]
fn a_saved_copy_is_written_back_and_a_change_on_the_host_asks_first() {
    let Some((mut rig, pane)) = connected("edit") else {
        return;
    };
    let remote = rig.dir.join("it's a file.txt");
    std::fs::write(&remote, "remote v1\n").expect("remote file");
    rig.hub
        .edit_remote(Some(pane), &format!("fake:{}", remote.display()));
    let opened = rig.files.clone();
    rig.pump("the editor to open", move |_| !opened.0.borrow().is_empty());
    let local = rig.files.0.borrow()[0].clone();
    assert_eq!(
        std::fs::read_to_string(&local).expect("copy"),
        "remote v1\n"
    );
    let id = EditId(1);

    // A save in the editor goes back to the host.
    std::fs::write(&local, "edited here\n").expect("save");
    until_remote(&mut rig, &remote, "edited here\n");

    // The host's file changes behind the editor's back; the next save asks.
    std::fs::write(&remote, "changed there, longer\n").expect("remote change");
    std::fs::write(&local, "edited again, here\n").expect("save");
    rig.pump("the question", |hub| {
        hub.host_view(pane)
            .is_some_and(|v| v.edits.iter().any(|e| e.status == EditStatus::Changed))
    });
    assert_eq!(
        std::fs::read_to_string(&remote).expect("remote"),
        "changed there, longer\n",
        "nothing is written before the person chooses"
    );
    rig.hub.resolve_edit(id, Resolution::Overwrite);
    until_remote(&mut rig, &remote, "edited again, here\n");
}

#[test]
fn a_file_can_only_be_edited_over_a_connection_that_is_up() {
    let Some(mut rig) = Rig::new("edit-down") else {
        return;
    };
    rig.hub.edit_remote(None, "fake:/etc/hosts");
    rig.hub.edit_remote(None, "no host here");
    let said = std::mem::take(&mut rig.hub.notices);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].contains("Open a session on fake first"), "{said:?}");
    assert!(said[1].contains("computer:/folder/file"), "{said:?}");
}

#[test]
fn a_forwarded_port_opens_on_a_free_port_and_closes() {
    let Some((mut rig, pane)) = connected("forward") else {
        return;
    };
    rig.hub.forward_port(pane, "not a port");
    assert!(
        rig.hub
            .notices
            .pop()
            .is_some_and(|n| n.contains("A port is a number"))
    );
    rig.hub.forward_port(pane, "3000");
    let port = Port::parse("3000").expect("port");
    rig.pump("the forward", move |hub| {
        hub.host_view(pane).is_some_and(|v| {
            v.forwards
                .iter()
                .any(|f| f.remote == port && f.local.is_some())
        })
    });
    let log = rig.got("control.log").unwrap_or_default();
    assert!(log.contains("-L"), "{log}");
    assert!(log.contains(":localhost:3000"), "{log}");
    // The click opens it in the browser, on the local port.
    rig.hub.open_forwarded(pane, port);
    let local = rig
        .hub
        .host_view(pane)
        .and_then(|v| v.forwards[0].local)
        .expect("local port");
    assert_eq!(
        *rig.links.0.borrow(),
        [format!("http://localhost:{}", local.get())]
    );

    rig.hub.cancel_forward(pane, port);
    rig.pump("the forward to close", |hub| {
        hub.host_view(pane).is_some_and(|v| v.forwards.is_empty())
    });
}

#[test]
fn a_forward_the_host_refuses_is_told_to_the_person() {
    let Some((mut rig, pane)) = connected("forward-refused") else {
        return;
    };
    rig.put("control-exit", "1");
    rig.hub.forward_port(pane, "3000");
    rig.pump("the refusal", |hub| !hub.notices.is_empty());
    assert!(
        rig.hub.notices[0].contains("wouldn't forward port 3000"),
        "{:?}",
        rig.hub.notices
    );
    assert!(
        rig.hub
            .host_view(pane)
            .is_some_and(|v| v.forwards.is_empty())
    );
}

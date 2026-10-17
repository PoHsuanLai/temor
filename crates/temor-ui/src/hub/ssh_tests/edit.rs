//! Editing a host's file locally and forwarding its ports, through the stand-in `ssh` and `scp`
//! (the stand-in `ssh` runs the file calls on this computer, so the "host" is a scratch folder).

use temor_core::host::edit::{Edit, EditIn, Found, Queued, Resolution};
use temor_core::remote::Grant;
use temor_core::workspace::WorkspaceIn;
use temor_ssh::Port;

use super::support::Rig;
use crate::edge::ssh_work::EditId;
use crate::hub::ssh::DoneAsked;
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

    // Done while a changed file waits for the person's choice keeps the copy.
    let stat = temor_ssh::RemoteStat {
        mtime: temor_ssh::UnixSeconds(1),
        size: temor_ssh::ByteCount(1),
    };
    let kept = rig.hub.ssh.edits.get_mut(&id).map(|edit| {
        let before = edit.state;
        edit.state = Edit::Conflict {
            base: stat,
            found: Found::Deleted,
        };
        before
    });
    rig.hub.close_edit(id);
    assert!(local.exists(), "the copy stays while a conflict is pending");
    if let Some(edit) = rig.hub.ssh.edits.get_mut(&id)
        && let Some(before) = kept
    {
        edit.state = before;
    }

    // Done while a save is on its way waits for it. A save that does not get there keeps the
    // copy (and Done has to be pressed again).
    let uploading = Edit::Uploading {
        base: stat,
        queued: Queued::Nothing,
    };
    let set = |rig: &mut Rig, state: Edit| {
        if let Some(edit) = rig.hub.ssh.edits.get_mut(&id) {
            edit.state = state;
        }
    };
    set(&mut rig, uploading);
    rig.hub.close_edit(id);
    assert!(local.exists(), "Done waits for the save on its way");
    rig.hub.on_edit(id, EditIn::Conflicted(Found::Deleted));
    assert!(
        local.exists(),
        "a save that did not get there keeps the copy"
    );
    assert_eq!(
        rig.hub.ssh.edits.get(&id).map(|edit| edit.done),
        Some(DoneAsked::No),
        "the earlier Done does not carry over"
    );

    // Done: once the save is on the host, the copy goes, and so does the folder made for it.
    set(&mut rig, uploading);
    rig.hub.close_edit(id);
    assert!(local.exists(), "still on its way");
    rig.hub.on_edit(id, EditIn::Stored { stat });
    assert!(
        !local.exists(),
        "the copy is deleted once the save is on the host"
    );
    assert!(
        local.parent().is_some_and(|dir| !dir.exists()),
        "and its folder"
    );
}

#[test]
fn a_file_of_a_host_that_is_not_connected_connects_first_and_opens_when_it_is_up() {
    let Some(mut rig) = Rig::new("edit-down") else {
        return;
    };
    rig.hub.edit_remote(None, "no host here");
    assert!(
        rig.hub.notices[0].contains("computer:/folder/file"),
        "{:?}",
        rig.hub.notices
    );
    rig.hub.notices.clear();
    rig.hub.edit_remote(None, "[2001:db8::1]:/etc/hosts");
    assert!(
        rig.hub.notices[0].contains("HostName"),
        "{:?}",
        rig.hub.notices
    );
    rig.hub.notices.clear();

    let remote = rig.dir.join("later.txt");
    std::fs::write(&remote, "there\n").expect("remote file");
    rig.hub
        .edit_remote(None, &format!("fake:{}", remote.display()));
    assert!(
        rig.hub.notices[0].contains("Connecting to fake"),
        "{:?}",
        rig.hub.notices
    );
    // A tab of the host opened; the person answers its questions as for any host.
    let pane = rig.front();
    assert_eq!(rig.hub.host_of(pane), Some("fake"));
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    assert!(rig.files.0.borrow().is_empty(), "nothing opens before then");
    let dir = rig.dir.clone();
    rig.pump("the first session", |_| dir.join("session-1").exists());
    rig.put("up", "");
    let opened = rig.files.clone();
    rig.pump("the editor to open", move |_| !opened.0.borrow().is_empty());
    let local = rig.files.0.borrow()[0].clone();
    assert_eq!(std::fs::read_to_string(local).expect("copy"), "there\n");
}

#[test]
fn a_reopened_tab_is_a_session_on_the_same_host() {
    let Some((mut rig, pane)) = connected("reopen") else {
        return;
    };
    assert_eq!(rig.hub.host_of(pane), Some("fake"));
    let tab = rig.hub.ws.tabs.place.tabs[0].id;
    // A second tab keeps the window from starting a fresh one in place of the closed tab.
    rig.hub.act(WorkspaceIn::NewTab { cwd: None });
    rig.hub.act(WorkspaceIn::CloseTab(tab));
    assert_eq!(
        rig.hub.host_of(pane),
        None,
        "closed, so no longer a session"
    );
    rig.hub.act(WorkspaceIn::Reopen);
    assert_eq!(rig.hub.host_of(pane), Some("fake"));
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
    // Whether this computer has the port free or not (an "in use" notice may come first), the
    // host's refusal is said.
    rig.hub.forward_port(pane, "3000");
    let refused = |notices: &[String]| {
        notices
            .iter()
            .any(|n| n.contains("wouldn't forward port 3000"))
    };
    rig.pump("the refusal", move |hub| refused(&hub.notices));
    assert!(
        rig.hub
            .host_view(pane)
            .is_some_and(|v| v.forwards.is_empty())
    );
}

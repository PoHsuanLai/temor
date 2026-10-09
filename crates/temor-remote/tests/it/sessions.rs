//! Sessions: running, surviving a lost connection, and coming back with the exact screen.

use temor_remote::ClientEvent;
use temor_wire::message::{ExitStatus, IdleLimit, SessionState};
use temor_wire::units::{Cols, ExitCode, Rows, Seconds, Size};

use crate::support::{Rig, eventually, only_session, snapshot_text, tools_present};

#[test]
fn input_reaches_the_program_and_output_streams_back() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("stream");
    let mut conn = rig.connect();
    let id = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(id).expect("attach");
    conn.client
        .send_input(id, b"echo sum-$((6*7))\n")
        .expect("input");
    conn.watch
        .output_has("the product", |out| out.contains("sum-42"));
}

#[test]
fn reattaching_after_a_lost_connection_restores_the_screen_then_goes_live() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("reattach");
    let gate = rig.files.join("gate");
    let mut first = rig.connect();
    let id = first.client.open_session(rig.bash()).expect("open");
    first.client.attach(id).expect("attach");
    first
        .client
        .send_input(id, b"echo before-$((1+1))\n")
        .expect("input");
    first
        .watch
        .output_has("the first line", |out| out.contains("before-2"));
    // The program goes on printing after the connection is gone.
    let waiting = format!(
        "while [ ! -e {} ]; do sleep 0.05; done; echo during-$((4+4))\n",
        gate.display()
    );
    first
        .client
        .send_input(id, waiting.as_bytes())
        .expect("input");
    first
        .watch
        .output_has("the loop starting", |out| out.contains("while"));
    first.hang_up().expect("the daemon sees a clean end");

    let mut second = rig.connect();
    let listed = second.client.list_sessions().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, SessionState::Detached);

    std::fs::write(&gate, b"go").expect("open the gate");
    let attached = eventually("the output made while away", || {
        let attached = second.client.attach(id).ok()?;
        snapshot_text(&attached)
            .contains("during-8")
            .then_some(attached)
    });
    let screen = snapshot_text(&attached);
    assert!(screen.contains("before-2"), "history is kept:\n{screen}");

    second
        .client
        .send_input(id, b"echo live-$((5+5))\n")
        .expect("input");
    second
        .watch
        .output_has("live output", |out| out.contains("live-10"));
}

#[test]
fn a_listing_shows_the_folder_the_program_reports_and_who_is_attached() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("listing");
    let mut conn = rig.connect();
    let id = conn.client.open_session(rig.bash()).expect("open");
    let before = conn.client.list_sessions().expect("list");
    assert_eq!(
        (before[0].state, before[0].cwd.0.as_str()),
        (
            SessionState::Detached,
            rig.home.to_str().expect("utf-8 path")
        )
    );

    conn.client.attach(id).expect("attach");
    conn.client
        .send_input(
            id,
            b"printf '\\033]7;file://host/srv/a%%20b\\007'; echo reported\n",
        )
        .expect("input");
    conn.watch
        .output_has("the report", |out| out.contains("reported"));
    let after = eventually("the folder", || {
        let listed = conn.client.list_sessions().ok()?;
        (listed[0].cwd.0 == "/srv/a b").then_some(listed)
    });
    assert_eq!(after[0].state, SessionState::Attached);
}

#[test]
fn a_resize_reaches_both_the_program_and_the_listing() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("resize");
    let mut conn = rig.connect();
    let id = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(id).expect("attach");
    let size = Size {
        cols: Cols(100),
        rows: Rows(30),
    };
    conn.client.resize(id, size).expect("resize");
    conn.client.send_input(id, b"stty size\n").expect("input");
    conn.watch
        .output_has("the size", |out| out.contains("30 100"));
    assert_eq!(conn.client.list_sessions().expect("list")[0].size, size);

    let empty = Size {
        cols: Cols(0),
        rows: Rows(24),
    };
    assert!(conn.client.resize(id, empty).is_err());
}

#[test]
fn a_program_that_exits_is_reported_and_forgotten_and_close_ends_one_that_runs() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("exit");
    let mut conn = rig.connect();
    let leaving = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(leaving).expect("attach");
    conn.client.send_input(leaving, b"exit 3\n").expect("input");
    let status = conn.watch.event("the exit", |event| match event {
        ClientEvent::Exited { session, status } if *session == leaving => Some(status.clone()),
        _ => None,
    });
    assert_eq!(status, ExitStatus::Code(ExitCode(3)));
    eventually("the session to be forgotten", || {
        conn.client.list_sessions().ok()?.is_empty().then_some(())
    });

    let closed = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(closed).expect("attach");
    conn.client.close_session(closed).expect("close");
    conn.watch
        .event("the exit after close", |event| match event {
            ClientEvent::Exited { session, .. } if *session == closed => Some(()),
            _ => None,
        });
    eventually("the closed session to be forgotten", || {
        conn.client.list_sessions().ok()?.is_empty().then_some(())
    });
}

#[test]
fn only_detached_sessions_expire_and_only_after_the_whole_limit() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("idle");
    let conn = rig.connect();
    conn.client
        .set_idle_limit(IdleLimit::After(Seconds(60)))
        .expect("limit");
    let kept = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(kept).expect("attach");
    let expiring = conn.client.open_session(rig.bash()).expect("open");

    rig.clock.advance(temor_wire::link::Millis(59_000));
    rig.daemon.tick();
    assert_eq!(conn.client.list_sessions().expect("list").len(), 2);

    rig.clock.advance(temor_wire::link::Millis(2_000));
    rig.daemon.tick();
    eventually("the detached session to end", || {
        let listed = conn.client.list_sessions().ok()?;
        (listed.len() == 1 && listed[0].session == kept).then_some(())
    });
    assert_eq!(only_session(&conn), Some(kept));
    assert_ne!(kept, expiring);
}

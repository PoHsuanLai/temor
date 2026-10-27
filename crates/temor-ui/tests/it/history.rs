//! History as it happens (PLAN P23): a window keeps each shell's session and its finished
//! commands through the client while they run, never at quit, and never a command typed after a
//! space or one that holds a secret.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ds_harness::WindowHosting;
use temor_client::{Client, ClientConfig, SqliteStore};
use temor_ui::view::probe::Probe;

use super::drive::{type_line, wait_until};
use super::window::harness_building;

/// The commands kept for the one session, in the order it ran them.
fn kept(store: &SqliteStore) -> Vec<String> {
    let sessions = store.sessions(None, None, 10).expect("sessions");
    sessions
        .first()
        .map(|session| {
            store
                .commands_of(session.id)
                .expect("commands")
                .into_iter()
                .map(|ran| ran.command)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_shells_finished_commands_are_kept_as_they_finish_but_not_a_secret() {
    if !super::support::window_tools_present() {
        return;
    }
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = Client::open(store.clone(), ClientConfig { external: false });
    let probe = Probe::new();
    let given = client.clone();
    let mut h = harness_building(
        "history-kept",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        move |config| config.with_context(given),
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "echo kept-one");
    wait_until(&mut h, &probe, "its output", |s| s.matches("kept-one").count() >= 2);
    type_line(&mut h, "export SERVICE_TOKEN=abc");
    type_line(&mut h, "echo kept-two");
    let start = Instant::now();
    while kept(&store) != ["echo kept-one", "echo kept-two"] {
        h.advance(Duration::from_millis(50));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "kept: {:?}",
            kept(&store)
        );
    }
    let sessions = store.sessions(None, None, 10).expect("sessions");
    assert_eq!(sessions.len(), 1, "one shell, one session");
    assert!(sessions[0].ended.is_none(), "it still runs");
}

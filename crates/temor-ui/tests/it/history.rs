//! History as it happens (PLAN P23): a window keeps each shell's session and its finished
//! commands through the client while they run, never at quit, and never a command typed after a
//! space or one that holds a secret.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds_harness::WindowHosting;
use temor_client::{Client, ClientConfig, SqliteStore};
use temor_ui::view::probe::Probe;

use super::drive::{chord_with, press, type_line, wait_html, wait_until};
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
    wait_until(&mut h, &probe, "its output", |s| {
        s.matches("kept-one").count() >= 2
    });
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

/// ⌥⇧⌘Y opens the history on the row's stage: the session by day, and pressed, its commands with
/// a way back to its tab; Back returns to the list.
#[test]
fn the_history_page_lists_the_session_and_opens_its_commands() {
    if !super::support::window_tools_present() {
        return;
    }
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = Client::open(store.clone(), ClientConfig { external: false });
    let probe = Probe::new();
    let mut h = harness_building(
        "history-page",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        move |config| config.with_context(client),
    );
    h.resize_window(ds_blitz::Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "echo seen-in-history");
    let start = Instant::now();
    while kept(&store).is_empty() {
        h.advance(Duration::from_millis(50));
        assert!(start.elapsed() < Duration::from_secs(10), "nothing kept");
    }
    // ⌥⇧⌘Y, or Ctrl+Alt+Shift+Y off a Mac.
    let primary: &[ShortcutKey] = match cfg!(target_os = "macos") {
        true => &[ShortcutKey::Super, ShortcutKey::Alt, ShortcutKey::Shift],
        false => &[ShortcutKey::Ctrl, ShortcutKey::Alt, ShortcutKey::Shift],
    };
    chord_with(&mut h, primary, 'y');
    wait_html(&mut h, "the history page", |html| {
        html.contains("class=\"temor-history-day\"") && html.contains("Today")
    });
    press(&mut h, ".temor-history-list .ds-row");
    wait_html(&mut h, "the session's commands", |html| {
        html.contains("echo seen-in-history") && html.contains("Go to Tab")
    });
    chord_with(&mut h, &primary[..1], '[');
    wait_html(&mut h, "the list again", |html| {
        html.contains("class=\"temor-history-day\"") && !html.contains("Go to Tab")
    });
}

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
    // ⌥⇧⌘Y: the window reads Super as temor's primary key on every desktop.
    let primary: &[ShortcutKey] = &[ShortcutKey::Super, ShortcutKey::Alt, ShortcutKey::Shift];
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

/// A row's Show History opens its own session's commands.
#[test]
fn a_rows_show_history_opens_its_sessions_commands() {
    if !super::support::window_tools_present() {
        return;
    }
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = Client::open(store.clone(), ClientConfig { external: false });
    let probe = Probe::new();
    let mut h = harness_building(
        "history-row-menu",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        move |config| config.with_context(client),
    );
    h.resize_window(ds_blitz::Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "echo from-this-row");
    let start = Instant::now();
    while kept(&store).is_empty() {
        h.advance(Duration::from_millis(50));
        assert!(start.elapsed() < Duration::from_secs(10), "nothing kept");
    }
    let row = h
        .centre(".temor-side .temor-tab-box")
        .expect("the tab's row");
    h.send(ds_harness::Input::press(
        row,
        ds::base::press::PointerButton::Secondary,
    ));
    h.advance(Duration::from_millis(300));
    let item = super::drive::menu_item(&h, "Show History");
    h.send(ds_harness::Input::click(item.centre()));
    wait_html(&mut h, "the session's commands", |html| {
        html.contains("echo from-this-row") && html.contains("Go to Tab")
    });
}

/// A pane brought back at launch goes on with its session: the commands of both launches are
/// one session's, numbered one after another.
#[test]
fn a_pane_brought_back_goes_on_with_its_session() {
    if !super::support::window_tools_present() {
        return;
    }
    let state = super::support::scratch("history-carried-state").join("state");
    let db = state.join("temor.db");
    let commands = |db: &std::path::Path| -> (usize, Vec<(u64, String)>) {
        let store = SqliteStore::open(db).expect("store");
        let sessions = store.sessions(None, None, 10).expect("sessions");
        let ran = sessions
            .first()
            .map(|session| store.commands_of(session.id).expect("commands"))
            .unwrap_or_default();
        (
            sessions.len(),
            ran.into_iter()
                .map(|ran| (ran.block.0, ran.command))
                .collect(),
        )
    };
    let probe = Probe::new();
    let mut h = super::window::harness_in("history-carried", &probe, Some(state.clone()));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "echo before-restart");
    let start = Instant::now();
    while commands(&db).1.is_empty() {
        h.advance(Duration::from_millis(50));
        assert!(start.elapsed() < Duration::from_secs(10), "nothing kept");
    }
    drop(h);

    let probe = Probe::new();
    let mut h = super::window::harness_in("history-carried-again", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "echo after-restart");
    let start = Instant::now();
    loop {
        let (sessions, ran) = commands(&db);
        if ran.len() == 2 {
            assert_eq!(sessions, 1, "one session: {ran:?}");
            assert_eq!(ran[0].1, "echo before-restart");
            assert_eq!(ran[1].1, "echo after-restart");
            assert!(ran[1].0 > ran[0].0, "numbered after: {ran:?}");
            break;
        }
        h.advance(Duration::from_millis(50));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{sessions} sessions, {ran:?}"
        );
    }
}

/// A closed session's Reopen starts a tab in its folder that goes on as that session: the
/// commands it ran are cards above the shell, with their output not kept, and Put in Prompt
/// types one again; run, it is the session's next command.
#[test]
fn a_reopened_session_shows_its_commands_as_cards_and_goes_on() {
    use temor_schema::session::{At, RanCommand, Session, SessionId};
    use temor_schema::url::Host;
    if !super::support::window_tools_present() {
        return;
    }
    let folder = super::support::scratch("history-reopen-folder");
    let folder = folder.to_string_lossy().into_owned();
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let id = SessionId::from_bits(0xb5);
    let day = 24 * 60 * 60 * 1000;
    let then = temor_ui::edge::local_zone::wall_now().0 - 2 * day;
    store
        .save_session(&Session {
            id,
            host: Host::Local,
            folder: folder.clone(),
            program: None,
            started: At(then),
            ended: Some(At(then + 60_000)),
        })
        .expect("session");
    for (block, command, exit) in [(0, "echo was-kept", 0), (1, "false", 1)] {
        store
            .record_command(&RanCommand {
                session: id,
                block: temor_core::vocab::ids::BlockId(block),
                command: command.to_owned(),
                folder: folder.clone(),
                exit: Some(exit),
                started: At(then + 1_000),
                ended: Some(At(then + 1_500)),
            })
            .expect("command");
    }
    let client = Client::open(store.clone(), ClientConfig { external: false });
    let probe = Probe::new();
    let mut h = harness_building(
        "history-reopen",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        move |config| config.with_context(client),
    );
    h.resize_window(ds_blitz::Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let primary: &[ShortcutKey] = &[ShortcutKey::Super, ShortcutKey::Alt, ShortcutKey::Shift];
    chord_with(&mut h, primary, 'y');
    wait_html(&mut h, "the closed session", |html| {
        html.contains("class=\"temor-history-day\"") && html.contains("history-reopen-folder")
    });
    // The closed session is the oldest: the list's last row.
    let rows = super::drive::rects(&h, ".temor-history-list .ds-row");
    let last = rows.last().expect("a row");
    h.send(ds_harness::Input::click(last.centre()));
    h.advance(Duration::from_millis(200));
    wait_html(&mut h, "its page", |html| html.contains("Reopen"));
    press(&mut h, ".temor-history-head .ds-button");
    wait_html(
        &mut h,
        "its commands as cards, the composer under them",
        |html| {
            html.matches("temor-blk-kept").count() >= 2
                && html.contains("exit 1")
                && html.contains("temor-blk-composer")
        },
    );
    let put = super::drive::rects(&h, ".temor-blk-kept .temor-blk-tools");
    h.send(ds_harness::Input::click(put[0].centre()));
    wait_html(&mut h, "the kept command in the composer", |html| {
        super::block_list::composer_value(html).as_deref() == Some("echo was-kept")
    });
    h.send(ds_harness::Input::key(ShortcutKey::Enter));
    let start = Instant::now();
    loop {
        let ran = store.commands_of(id).expect("commands");
        let session = store.session(id).expect("session").expect("kept");
        if ran.len() == 3 {
            assert_eq!(ran[2].command, "echo was-kept");
            assert!(ran[2].block.0 > ran[1].block.0, "numbered after: {ran:?}");
            assert!(session.ended.is_none(), "it runs again");
            assert_eq!(session.started, At(then), "the same session");
            break;
        }
        h.advance(Duration::from_millis(50));
        assert!(start.elapsed() < Duration::from_secs(10), "{ran:?}");
    }
}

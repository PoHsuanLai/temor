use temor_schema::url::{HistoryScope, Location};

use super::*;
use crate::Committed;

const ML: &str = "/Users/po/ml";

fn store() -> SqliteStore {
    SqliteStore::in_memory().expect("store")
}

fn session(bits: u64, folder: &str, started: i64) -> Session {
    Session {
        id: SessionId::from_bits(bits),
        host: Host::Local,
        folder: folder.to_owned(),
        program: None,
        started: At(started),
        ended: None,
    }
}

fn ran(bits: u64, block: u64, command: &str, folder: &str, started: i64) -> RanCommand {
    RanCommand {
        session: SessionId::from_bits(bits),
        block: BlockId(block),
        command: command.to_owned(),
        folder: folder.to_owned(),
        exit: None,
        started: At(started),
        ended: None,
    }
}

fn commands(found: &[RanCommand]) -> Vec<&str> {
    found.iter().map(|c| c.command.as_str()).collect()
}

fn place(path: &str) -> TemorUrl {
    TemorUrl::Location(Location {
        host: Host::Local,
        path: path.to_owned(),
        within: None,
    })
}

#[test]
fn a_session_reads_back_as_kept_and_its_high_bits_survive() {
    let store = store();
    let mut kept = session(u64::MAX - 1, ML, 10);
    kept.host = Host::named("box").expect("host");
    kept.program = Some("claude".to_owned());
    store.save_session(&kept).expect("save");
    assert_eq!(store.session(kept.id).expect("read"), Some(kept.clone()));
    store.end_session(kept.id, At(20)).expect("end");
    assert_eq!(
        store.session(kept.id).expect("read").and_then(|s| s.ended),
        Some(At(20))
    );
}

#[test]
fn sessions_are_newest_first_narrowed_to_a_folder_and_paged() {
    let store = store();
    store.save_session(&session(1, ML, 10)).expect("save");
    store
        .save_session(&session(2, "/Users/po/ml/data", 20))
        .expect("save");
    store
        .save_session(&session(3, "/Users/po/mlx", 30))
        .expect("save");
    let ids = |found: Vec<Session>| found.iter().map(|s| s.id.bits()).collect::<Vec<_>>();
    assert_eq!(ids(store.sessions(None, None, 10).expect("all")), [3, 2, 1]);
    assert_eq!(
        ids(store.sessions(Some("/Users/po/ml/"), None, 10).expect("in")),
        [2, 1]
    );
    assert_eq!(
        ids(store.sessions(Some("/"), None, 10).expect("root")),
        [3, 2, 1]
    );
    assert_eq!(
        ids(store.sessions(None, Some(At(30)), 1).expect("page")),
        [2]
    );
}

#[test]
fn a_command_is_kept_once_and_then_told_how_it_ended() {
    let store = store();
    store.save_session(&session(1, ML, 0)).expect("save");
    store
        .record_command(&ran(1, 1, "ls", ML, 5))
        .expect("record");
    let mut done = ran(1, 1, "ls", ML, 5);
    done.exit = Some(0);
    done.ended = Some(At(6));
    store.record_command(&done).expect("again");
    store
        .record_command(&ran(1, 2, "pwd", ML, 7))
        .expect("record");
    let kept = store.commands_of(SessionId::from_bits(1)).expect("read");
    assert_eq!(kept, [done, ran(1, 2, "pwd", ML, 7)]);
}

/// A command history may not keep is not written, by whoever writes to the store.
#[test]
fn a_secret_or_a_command_typed_after_a_space_is_never_written() {
    let store = store();
    store.save_session(&session(1, ML, 0)).expect("save");
    for (block, line) in [
        (1, " curl -u me:hunter2 example.com"),
        (2, "export GITHUB_TOKEN=ghp_0123456789abcdefghijklmn"),
        (3, "ls"),
    ] {
        store
            .record_command(&ran(1, block, line, ML, 1))
            .expect("record");
    }
    let kept = store.commands_of(SessionId::from_bits(1)).expect("read");
    assert_eq!(commands(&kept), ["ls"]);
}

#[test]
fn a_command_needs_its_session_and_goes_with_it() {
    let store = store();
    assert!(store.record_command(&ran(9, 1, "ls", ML, 0)).is_err());
    store.save_session(&session(1, ML, 0)).expect("save");
    store
        .record_command(&ran(1, 1, "ls", ML, 1))
        .expect("record");
    store
        .forget_session(SessionId::from_bits(1))
        .expect("forget");
    let found = store
        .search(&Found {
            limit: 10,
            ..Found::default()
        })
        .expect("search");
    assert!(found.is_empty());
}

#[test]
fn search_finds_any_part_of_a_command_any_case_and_short_text_too() {
    let store = store();
    store.save_session(&session(1, ML, 0)).expect("save");
    store
        .record_command(&ran(1, 1, "cargo nextest run", ML, 1))
        .expect("record");
    store
        .record_command(&ran(1, 2, "git status", "/tmp", 2))
        .expect("record");
    store
        .record_command(&ran(1, 3, "echo \"quoted\" OR x", ML, 3))
        .expect("record");
    let find = |text: &str, folder: Option<&str>| {
        store
            .search(&Found {
                text: Some(text),
                folder,
                before: None,
                limit: 10,
            })
            .expect("search")
    };
    assert_eq!(commands(&find("NEXTEST", None)), ["cargo nextest run"]);
    assert_eq!(
        commands(&find("st", None)),
        ["git status", "cargo nextest run"]
    );
    assert_eq!(
        commands(&find("\"quoted\" OR", None)),
        ["echo \"quoted\" OR x"]
    );
    assert_eq!(
        commands(&find("ml", None)).len(),
        2,
        "a folder is searched too"
    );
    assert_eq!(commands(&find("st", Some("/tmp/"))), ["git status"]);
}

#[test]
fn search_pages_by_start() {
    let store = store();
    store.save_session(&session(1, ML, 0)).expect("save");
    for n in 1..=5 {
        store
            .record_command(&ran(
                1,
                n,
                &format!("make {n}"),
                ML,
                i64::try_from(n).expect("n"),
            ))
            .expect("record");
    }
    let page = store
        .search(&Found {
            text: Some("make"),
            folder: None,
            before: Some(At(4)),
            limit: 2,
        })
        .expect("page");
    assert_eq!(commands(&page), ["make 3", "make 2"]);
}

#[test]
fn old_history_is_forgotten_and_a_running_session_is_not() {
    let store = store();
    let mut old = session(1, ML, 0);
    old.ended = Some(At(5));
    store.save_session(&old).expect("save");
    store
        .record_command(&ran(1, 1, "ls", ML, 1))
        .expect("record");
    store.save_session(&session(2, ML, 0)).expect("running");
    store
        .record_command(&ran(2, 1, "old", ML, 1))
        .expect("record");
    store
        .record_command(&ran(2, 2, "new", ML, 200))
        .expect("record");
    assert_eq!(store.forget_before(At(100)).expect("forget"), 1);
    assert_eq!(store.session(old.id).expect("read"), None);
    assert_eq!(
        commands(&store.commands_of(SessionId::from_bits(2)).expect("read")),
        ["new"]
    );
}

#[test]
fn a_shelf_keeps_its_order_and_each_place_once() {
    let store = store();
    let places = [
        place("/b/"),
        place("/a/"),
        place("/b/"),
        TemorUrl::History(HistoryScope::All),
    ];
    store.set_shelf(Shelf::Pins, &places).expect("set");
    assert_eq!(
        store.shelf(Shelf::Pins).expect("read"),
        [
            place("/b/"),
            place("/a/"),
            TemorUrl::History(HistoryScope::All)
        ]
    );
    assert!(store.shelf(Shelf::Favourites).expect("read").is_empty());
    store.set_shelf(Shelf::Pins, &[place("/a/")]).expect("set");
    assert_eq!(store.shelf(Shelf::Pins).expect("read"), [place("/a/")]);
}

#[test]
fn a_kept_value_is_replaced() {
    let store = store();
    assert_eq!(store.kept("window").expect("read"), None);
    store
        .keep("window", "{\"rows\":1}", Topics::LAYOUT)
        .expect("keep");
    store
        .keep("window", "{\"rows\":2}", Topics::LAYOUT)
        .expect("keep");
    assert_eq!(
        store.kept("window").expect("read").as_deref(),
        Some("{\"rows\":2}")
    );
}

#[test]
fn each_write_says_what_it_touched_once_it_is_committed() {
    let store = store();
    let heard = store.subscribe();
    store.save_session(&session(1, ML, 0)).expect("save");
    store
        .set_shelf(Shelf::Favourites, &[place("/a/")])
        .expect("set");
    // Ending a session that is not kept changes no row, and is not said.
    store
        .end_session(SessionId::from_bits(9), At(1))
        .expect("end");
    store.keep("hosts", "[]", Topics::HOSTS).expect("keep");
    let said: Vec<Committed> = heard.try_iter().collect();
    let topics: Vec<Topics> = said.iter().map(|c| c.topics).collect();
    assert_eq!(
        topics,
        [Topics::SESSIONS, Topics::FAVOURITES, Topics::HOSTS]
    );
    let generations: Vec<u64> = said.iter().map(|c| c.generation.0).collect();
    assert_eq!(generations, [1, 2, 3]);
    assert_eq!(store.generation().0, 3);
}

#[test]
fn a_file_store_is_private_read_beside_its_writer_and_seen_by_another() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("temor.db");
    let one = SqliteStore::open(&path).expect("one");
    assert_eq!(one.reader_count(), 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let two = SqliteStore::open(&path).expect("two");
    let before = one.data_version().expect("version");
    two.save_session(&session(1, ML, 0)).expect("save");
    assert_ne!(one.data_version().expect("version"), before);
    assert!(
        one.session(SessionId::from_bits(1))
            .expect("read")
            .is_some()
    );
    let pinned = one
        .read_pinned(|| {
            let first = one.sessions(None, None, 10).expect("first").len();
            two.save_session(&session(2, ML, 1)).expect("save");
            (first, one.sessions(None, None, 10).expect("second").len())
        })
        .expect("pinned");
    assert_eq!(pinned, (1, 1), "one read, one moment");
    assert_eq!(one.sessions(None, None, 10).expect("after").len(), 2);
}

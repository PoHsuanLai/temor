use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ds_style::space::list::{SpaceId, Spaces};
use temor_hosts::host::store::HostsFile;
use temor_layout::space::Project;
use temor_layout::workspace::WorkspaceIn;
use temor_layout::workspace::restore::encode;
use temor_store::{SqliteStore, Topics};

use super::{Kept, Laying};
use crate::Client;
use crate::keep::older;
use crate::spaces::Book;

fn client(dir: &tempfile::TempDir) -> Client {
    let store = SqliteStore::open(dir.path().join("temor.db")).expect("store");
    Client::inline(Arc::new(store))
}

/// Two Spaces, the first with two tabs and the second with one.
fn two_spaces() -> Book {
    let mut book = Book::new();
    book.begin(Spaces::first_run(
        [Project::default(), Project::default()],
        Project::default,
    ));
    book.step(WorkspaceIn::NewTab { cwd: None });
    book.step(WorkspaceIn::NewTab { cwd: None });
    book.swap_to(SpaceId(1));
    book.step(WorkspaceIn::NewTab { cwd: None });
    book.swap_to(SpaceId(0));
    book
}

fn lay(kept: &mut Kept, client: &Client, book: &Book) {
    let laying = Laying {
        book,
        leave: &BTreeSet::new(),
        hosts: &HostsFile::default(),
        sessions: &BTreeMap::new(),
    };
    kept.lay_all(client, laying);
}

#[test]
fn every_space_kept_comes_back_with_its_panes_to_start() {
    let dir = tempfile::tempdir().expect("dir");
    let client = client(&dir);
    let book = two_spaces();
    lay(&mut Kept::default(), &client, &book);

    let mut back = Book::new();
    let mut kept = Kept::default();
    let starting = kept.restore(Some(&client), &mut back);
    let panes: Vec<(SpaceId, usize)> = starting
        .iter()
        .map(|start| (start.space, start.panes.len()))
        .collect();
    assert_eq!(panes, vec![(SpaceId(0), 2), (SpaceId(1), 1)]);
    assert_eq!(back.spaces().count(), 2);
    assert_eq!(back.ws().tabs.panes(), book.ws().tabs.panes());
    assert_eq!(back.owner_of(starting[1].panes[0].pane), Some(SpaceId(1)));
    assert!(kept.take_notices().is_empty());
}

#[test]
fn a_space_sharing_a_pane_with_one_brought_back_is_left_out_and_said() {
    let dir = tempfile::tempdir().expect("dir");
    let client = client(&dir);
    let book = two_spaces();
    lay(&mut Kept::default(), &client, &book);
    let first = encode(book.ws()).expect("encoded");
    client
        .store()
        .keep("space/1", &first, Topics::LAYOUT)
        .expect("kept");

    let mut kept = Kept::default();
    let starting = kept.restore(Some(&client), &mut Book::new());
    assert_eq!(starting.len(), 1);
    assert_eq!(kept.take_notices().len(), 1);
}

#[test]
fn spaces_kept_unreadable_start_one_space_and_say_so() {
    let dir = tempfile::tempdir().expect("dir");
    let client = client(&dir);
    client
        .store()
        .keep("spaces", "{ not spaces", Topics::LAYOUT)
        .expect("kept");
    let mut kept = Kept::default();
    let mut book = Book::new();
    kept.restore(Some(&client), &mut book);
    assert_eq!(book.spaces().count(), 1);
    assert_eq!(kept.take_notices().len(), 1);
}

#[test]
fn an_older_temors_files_are_read_in_once() {
    let dir = tempfile::tempdir().expect("dir");
    let state = dir.path().join("state");
    let client = client(&dir);
    let book = two_spaces();
    older::save_spaces(&state, &book.for_keeping()).expect("spaces");
    older::save(&state, book.ws()).expect("session");

    let mut kept = Kept::default();
    kept.import(&client, &state);
    let mut back = Book::new();
    let starting = kept.restore(Some(&client), &mut back);
    assert_eq!(starting[0].panes.len(), 2);

    // A file written after the import is not read again.
    older::save(&state, &crate::spaces::empty_workspace()).expect("session");
    kept.import(&client, &state);
    let starting = Kept::default().restore(Some(&client), &mut Book::new());
    assert_eq!(starting[0].panes.len(), 2);
}

#[test]
fn without_a_client_a_space_comes_back_where_the_spaces_recall_it() {
    let book = two_spaces();
    let mut back = Book::new();
    back.begin(book.for_keeping());
    let starting = Kept::default().restore(None, &mut back);
    assert_eq!(starting.len(), 2, "both Spaces recall a tab");
}

/// What is kept at quit is written after every change still waiting on the client's thread, so
/// an older layout handed over before it never lands over it.
#[test]
fn the_layout_written_at_quit_lands_after_the_ones_still_waiting() {
    let dir = tempfile::tempdir().expect("dir");
    let store = SqliteStore::open(dir.path().join("temor.db")).expect("store");
    let client = Client::open(Arc::new(store), crate::ClientConfig { external: false });
    let book = two_spaces();
    let mut kept = Kept::default();
    // Changes handed to the client's thread, the last of them an older layout of the Spaces.
    for n in 0..200 {
        client.act(crate::Command::Lay {
            key: format!("other/{n}"),
            value: Some("x".to_owned()),
        });
    }
    client.act(crate::Command::Lay {
        key: "spaces".to_owned(),
        value: Some("older".to_owned()),
    });
    let laying = Laying {
        book: &book,
        leave: &BTreeSet::new(),
        hosts: &HostsFile::default(),
        sessions: &BTreeMap::new(),
    };
    kept.lay_at_quit(&client, laying);
    let spaces = client.store().kept("spaces").expect("read").expect("kept");
    assert_ne!(spaces, "older");
    assert_eq!(
        client.store().kept("other/199").expect("read").as_deref(),
        Some("x")
    );
}

#[test]
fn only_what_changed_is_written_and_a_gone_spaces_rows_are_forgotten() {
    use super::super::kept::Lay;
    let mut kept = Kept::default();
    for (key, text) in [
        ("space/1", "a"),
        ("space/2", "b"),
        ("hosts", "h"),
        ("spaces", "s"),
    ] {
        kept.written.insert(key.to_owned(), text.to_owned());
    }
    kept.newer.insert("space/9".to_owned());
    let plan: BTreeMap<String, Lay> = [
        ("space/1", Lay::Write("a".to_owned())),
        ("space/3", Lay::Write("c".to_owned())),
        ("space/9", Lay::Write("mine".to_owned())),
        ("hosts", Lay::Forget),
        ("spaces", Lay::Leave),
        ("sessions", Lay::Forget),
    ]
    .into_iter()
    .map(|(key, lay)| (key.to_owned(), lay))
    .collect();
    let mut changes = kept.changes(&plan);
    changes.sort();
    assert_eq!(
        changes,
        vec![
            ("hosts".to_owned(), None),
            ("space/2".to_owned(), None),
            ("space/3".to_owned(), Some("c".to_owned())),
        ],
        "unchanged rows, a key left alone, a newer temor's key and one never kept are not written"
    );
}

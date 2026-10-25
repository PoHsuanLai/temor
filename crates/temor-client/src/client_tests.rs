//! The client over real stores: values shared, read again when a commit touches what they read
//! and not otherwise, gathered, dropped, and heard from another process.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use temor_schema::ids::BlockId;
use temor_schema::session::{At, RanCommand, Session, SessionId};
use temor_schema::url::{Host, Location, TemorUrl};
use temor_store::{Shelf, SqliteStore};

use crate::query::{Commands, History, OnShelf, Sessions};
use crate::{Client, ClientConfig, ClientError, Live, Query, Snapshot, Topics};

fn client(store: &Arc<SqliteStore>) -> Client {
    Client::open(store.clone(), ClientConfig::default())
}

/// A store on disk, so it has readers beside its writer, and the directory it is in.
fn on_disk() -> (Arc<SqliteStore>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dir");
    let store = SqliteStore::open(dir.path().join("temor.db")).expect("store");
    (Arc::new(store), dir)
}

/// Wait until `done` holds, or fail saying `what`.
fn until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "{what} did not happen");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Wait until nothing has been read for a while, so a count of readings is final.
fn settled(client: &Client) {
    until("the client settling", || client.idle());
    std::thread::sleep(Duration::from_millis(250));
    until("the client settling", || client.idle());
}

fn place(path: &str) -> TemorUrl {
    TemorUrl::Location(Location {
        host: Host::Local,
        path: path.to_owned(),
        within: None,
    })
}

/// Pin `n` places.
fn pin(store: &SqliteStore, n: usize) {
    let places: Vec<TemorUrl> = (0..n).map(|i| place(&format!("/p{i}/"))).collect();
    store.set_shelf(Shelf::Pins, &places).expect("pin");
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

/// A query that counts its readings and reads how many places are pinned. Two are the same
/// query when their names are.
#[derive(Clone)]
struct Counted {
    name: &'static str,
    runs: Arc<AtomicUsize>,
    /// Held by a reading until something is sent, when there is one.
    gate: Option<Arc<Mutex<Receiver<()>>>>,
}

impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Counted {}

impl Counted {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            runs: Arc::new(AtomicUsize::new(0)),
            gate: None,
        }
    }

    /// One whose readings each wait for a word on the sender returned.
    fn gated(name: &'static str) -> (Self, Sender<()>) {
        let (tx, rx) = channel();
        let mut query = Self::new(name);
        query.gate = Some(Arc::new(Mutex::new(rx)));
        (query, tx)
    }

    fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }
}

impl Query for Counted {
    type Output = usize;
    const READS: Topics = Topics::PINS;

    fn run(&self, store: &SqliteStore) -> Result<usize, ClientError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.lock().expect("gate").recv().expect("opened");
        }
        Ok(store.shelf(Shelf::Pins)?.len())
    }
}

/// The same, reading only sessions: a pin is nothing to it. One of its kind.
#[derive(Clone)]
struct Unrelated(Arc<AtomicUsize>);

impl PartialEq for Unrelated {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Unrelated {}

impl Query for Unrelated {
    type Output = ();
    const READS: Topics = Topics::SESSIONS;

    fn run(&self, _store: &SqliteStore) -> Result<(), ClientError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn value<T: Send + Sync + 'static>(live: &Live<T>) -> Option<Arc<T>> {
    match live.snapshot() {
        Snapshot::Ready { value, .. } => Some(value),
        _ => None,
    }
}

#[test]
fn two_asking_the_same_query_share_one_value_and_one_reading() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let query = Counted::new("pins");
    let one = client.live(query.clone());
    let two = client.live(query.clone());
    assert_eq!(*one.wait(), 0);
    settled(&client);
    assert_eq!(query.runs(), 1);
    assert_eq!(value(&two).as_deref(), Some(&0));
}

#[test]
fn a_commit_reads_again_what_reads_what_it_touched_and_nothing_else() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let pins = client.live(Counted::new("pins"));
    let unrelated_runs = Arc::new(AtomicUsize::new(0));
    let _unrelated = client.live(Unrelated(unrelated_runs.clone()));
    pins.wait();
    settled(&client);

    pin(&store, 1);

    until("the pin counted", || value(&pins).as_deref() == Some(&1));
    settled(&client);
    assert_eq!(
        unrelated_runs.load(Ordering::SeqCst),
        1,
        "read again for pins"
    );
}

#[test]
fn commits_that_come_together_are_read_again_once_or_twice_not_each_time() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let query = Counted::new("pins");
    let pins = client.live(query.clone());
    pins.wait();
    settled(&client);

    for n in 1..=40 {
        pin(&store, n);
    }

    until("every pin counted", || value(&pins).as_deref() == Some(&40));
    settled(&client);
    let again = query.runs() - 1;
    assert!(
        (1..=3).contains(&again),
        "forty commits read the value again {again} times"
    );
}

#[test]
fn a_value_let_go_of_while_it_waits_is_not_read() {
    // In memory: one reading thread, which the gated query holds.
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = client(&store);
    let (busy, open) = Counted::gated("busy");
    let _busy = client.live(busy.clone());
    until("the busy reading to begin", || busy.runs() == 1);

    let waiting = Counted::new("waiting");
    drop(client.live(waiting.clone()));
    open.send(()).expect("open");
    settled(&client);

    assert_eq!(waiting.runs(), 0);
}

#[test]
fn a_value_that_goes_stale_while_it_is_read_is_read_again_after() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let (query, open) = Counted::gated("pins");
    let pins = client.live(query.clone());
    until("the first reading to begin", || query.runs() == 1);

    pin(&store, 1);
    std::thread::sleep(Duration::from_millis(250));
    open.send(()).expect("open");
    open.send(()).expect("open");

    until("the pin counted", || value(&pins).as_deref() == Some(&1));
    settled(&client);
    assert_eq!(query.runs(), 2);
}

#[test]
fn another_process_committing_is_heard() {
    let (store, dir) = on_disk();
    let client = client(&store);
    let sessions = client.live(Sessions {
        folder: None,
        before: None,
        limit: 10,
    });
    assert!(sessions.wait().is_empty());

    // Another connection to the same file: a second temor.
    let other = SqliteStore::open(dir.path().join("temor.db")).expect("other");
    other.save_session(&session(7, "/tmp", 1)).expect("save");

    until("the other process's session", || {
        value(&sessions).is_some_and(|now| now.len() == 1)
    });
}

#[test]
fn an_inline_client_has_the_value_at_once_and_reads_again_when_settled() {
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = Client::inline(store.clone());
    let pins = client.live(OnShelf(Shelf::Pins));
    assert_eq!(
        value(&pins).as_deref(),
        Some(&Vec::new()),
        "read as asked for"
    );

    pin(&store, 1);
    assert_eq!(
        value(&pins).map(|p| p.len()),
        Some(0),
        "nothing until settled"
    );
    client.settle();

    assert_eq!(value(&pins).as_deref(), Some(&vec![place("/p0/")]));
}

#[test]
fn a_change_is_told_and_the_same_answer_is_not() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let mut pins = client.live(Counted::new("pins"));
    pins.wait();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    // The first reading has been told already; take it.
    runtime.block_on(async {
        let _ = tokio::time::timeout(Duration::from_millis(50), pins.changed()).await;
    });

    pin(&store, 1);
    runtime.block_on(async {
        assert!(pins.changed().await);
    });
    // Pinned again, as it was: read again, and not news.
    pin(&store, 1);
    settled(&client);
    let told = runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(200), pins.changed())
            .await
            .is_ok()
    });
    assert!(!told, "the same count was told as a change");
}

#[test]
fn a_stopped_client_reads_nothing_more() {
    let (store, _dir) = on_disk();
    let client = client(&store);
    let query = Counted::new("pins");
    let pins = client.live(query.clone());
    pins.wait();
    client.clone().shutdown(Duration::from_secs(2));

    pin(&store, 1);
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(query.runs(), 1);
    assert_eq!(value(&pins).as_deref(), Some(&0));
}

#[test]
fn history_and_a_sessions_commands_follow_what_is_recorded() {
    let store = Arc::new(SqliteStore::in_memory().expect("store"));
    let client = Client::inline(store.clone());
    let id = SessionId::from_bits(1);
    let history = client.live(History {
        text: Some("cargo".to_owned()),
        folder: None,
        before: None,
        limit: 10,
    });
    let commands = client.live(Commands(id));
    assert_eq!(value(&commands).as_deref(), Some(&None));

    store.save_session(&session(1, "/w", 0)).expect("save");
    let ran = RanCommand {
        session: id,
        block: BlockId(1),
        command: "cargo build".to_owned(),
        folder: "/w".to_owned(),
        exit: Some(0),
        started: At(1),
        ended: Some(At(2)),
    };
    store.record_command(&ran).expect("record");
    client.settle();

    assert_eq!(value(&history).as_deref(), Some(&vec![ran.clone()]));
    assert_eq!(
        value(&commands).as_deref(),
        Some(&Some((session(1, "/w", 0), vec![ran])))
    );
}

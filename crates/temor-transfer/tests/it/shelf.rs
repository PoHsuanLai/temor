use std::path::PathBuf;

use temor_transfer::osc1337::{Osc1337Reader, ReceivedFile};
use temor_transfer::shelf::{
    Clock, Shelf, ShelfId, ShelfItem, ShelfStore, SystemClock, UnixMillis, candidate_name,
};
use temor_transfer::{Bytes, Consent, Count, FileName, HostName, Limits, Origin, TransferError};

struct FixedClock(i64);

impl Clock for FixedClock {
    fn now(&self) -> UnixMillis {
        UnixMillis::new(self.0)
    }
}

fn item(shelf: &Shelf, name: &str, at: i64, path: PathBuf) -> ShelfItem {
    ShelfItem {
        id: shelf.next_id(),
        name: FileName::sanitise(name),
        path,
        size: Bytes::new(3),
        origin: Origin::Host {
            host: HostName::new("prod-1"),
        },
        received_at: FixedClock(at).now(),
    }
}

#[test]
fn collisions_are_numbered_before_the_extension() {
    assert_eq!(candidate_name("report.pdf", 1), "report.pdf");
    assert_eq!(candidate_name("report.pdf", 2), "report (2).pdf");
    assert_eq!(candidate_name("archive.tar.gz", 3), "archive.tar (3).gz");
    assert_eq!(candidate_name("README", 2), "README (2)");
    assert_eq!(candidate_name(".bashrc", 2), ".bashrc (2)");
}

#[test]
fn the_shelf_is_newest_first_and_ids_are_not_reused() {
    let shelf = Shelf::default();
    let first = item(&shelf, "one", 100, PathBuf::from("/x/one"));
    let shelf = shelf.insert_item(first);
    let second = item(&shelf, "two", 200, PathBuf::from("/x/two"));
    let shelf = shelf.insert_item(second);
    let late = item(&shelf, "old", 150, PathBuf::from("/x/old"));
    let shelf = shelf.insert_item(late);
    let names: Vec<&str> = shelf.items().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["two", "old", "one"]);
    let (shelf, removed) = shelf.remove(ShelfId::new(2));
    assert_eq!(removed.map(|i| i.name), Some(FileName::sanitise("two")));
    assert_eq!(shelf.items().len(), 2);
    assert_eq!(shelf.next_id(), ShelfId::new(4));
}

#[test]
fn a_missing_index_is_an_empty_shelf() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ShelfStore::new(dir.path().join("downloads"));
    assert_eq!(
        store.load(&FixedClock(1_000)).expect("loads"),
        Shelf::default()
    );
}

#[test]
fn the_index_round_trips_and_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ShelfStore::new(dir.path());
    let shelf = Shelf::default().insert_item(item(
        &Shelf::default(),
        "a.txt",
        5,
        dir.path().join("a.txt"),
    ));
    store.save(&shelf).expect("saves");
    assert_eq!(store.load(&FixedClock(1_000)).expect("loads"), shelf);
    assert!(!dir.path().join("shelf.json.tmp").exists());
}

#[test]
fn placing_a_file_never_overwrites_and_moves_the_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let staging = tempfile::tempdir().expect("staging");
    let store = ShelfStore::new(dir.path().join("shelf"));
    let name = FileName::sanitise("report.pdf");
    let mut placed = Vec::new();
    for body in [&b"one"[..], &b"two"[..]] {
        let source = staging.path().join("incoming");
        std::fs::write(&source, body).expect("source");
        placed.push(store.place(&source, &name).expect("places"));
        assert!(!source.exists(), "the source is moved, not copied");
    }
    assert_eq!(
        placed[0].file_name().and_then(|n| n.to_str()),
        Some("report.pdf")
    );
    assert_eq!(
        placed[1].file_name().and_then(|n| n.to_str()),
        Some("report (2).pdf")
    );
    assert_eq!(std::fs::read(&placed[1]).expect("read"), b"two");
    assert_eq!(std::fs::read(&placed[0]).expect("read"), b"one");
}

#[test]
fn the_system_clock_reads_a_plausible_time() {
    assert!(SystemClock.now().get() > 1_700_000_000_000);
}

#[test]
fn an_unreadable_index_is_set_aside_so_a_save_cannot_destroy_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("shelf.json"), b"{not json").expect("write");
    let store = ShelfStore::new(dir.path());
    assert!(matches!(
        store.load(&FixedClock(4242)),
        Err(TransferError::Index { .. })
    ));
    assert!(!dir.path().join("shelf.json").exists());
    assert_eq!(
        std::fs::read(dir.path().join("shelf.json.corrupt-4242")).expect("set aside"),
        b"{not json"
    );
    store
        .save(&Shelf::default().insert_item(item(
            &Shelf::default(),
            "a.txt",
            1,
            dir.path().join("a.txt"),
        )))
        .expect("saves");
    assert_eq!(
        std::fs::read(dir.path().join("shelf.json.corrupt-4242")).expect("kept"),
        b"{not json"
    );
}

#[test]
fn a_consented_offer_is_placed_and_never_overwrites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ShelfStore::new(dir.path());
    let payload = b"1337;File=name=cmVwb3J0LnBkZg==:aGVsbG8=";
    let mut reader =
        Osc1337Reader::new(Limits::new(Bytes::new(1024), Count::new(1), Count::new(1)));
    let offer = match reader.feed(payload, false).expect("decodes") {
        Some(ReceivedFile::NeedsConsent(offer)) => offer,
        other => panic!("expected an offer, got {other:?}"),
    };
    let first = store
        .place_consented(&offer.clone().consent(Consent::Allow).expect("allowed"))
        .expect("places");
    let second = store
        .place_consented(&offer.consent(Consent::Allow).expect("allowed"))
        .expect("places");
    assert_eq!(
        first.file_name().and_then(|n| n.to_str()),
        Some("report.pdf")
    );
    assert_eq!(
        second.file_name().and_then(|n| n.to_str()),
        Some("report (2).pdf")
    );
    assert_eq!(std::fs::read(&second).expect("read"), b"hello");
}

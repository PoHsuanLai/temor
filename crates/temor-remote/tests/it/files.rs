//! The file service through the link: confinement, stamps, write-back and watches.

use std::os::unix::fs::symlink;

use temor_remote::{ClientEvent, Error};
use temor_wire::digest::ContentHash;
use temor_wire::message::{EntryKind, FileStamp, RemoteError, RemotePath, WatchChange, WriteCheck};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};

use crate::support::{Rig, tools_present};

fn span(offset: u64, len: u64) -> ByteRange {
    ByteRange {
        offset: ByteOffset(offset),
        len: ByteCount(len),
    }
}

fn remote(error: Error) -> RemoteError {
    match error {
        Error::Remote(error) => error,
        other => panic!("expected an error from the host, got {other:?}"),
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn listing_stat_and_ranged_reads() {
    let rig = Rig::new("files-read");
    std::fs::write(rig.files.join("a.txt"), b"abc").expect("file");
    std::fs::create_dir(rig.files.join("sub")).expect("folder");
    let conn = rig.connect();

    let listing = conn.client.list_dir(&rig.remote("")).expect("list");
    let seen: Vec<(&str, EntryKind, u64)> = listing
        .iter()
        .map(|e| (e.name.as_str(), e.kind, e.len.0))
        .collect();
    assert_eq!(seen[0], ("a.txt", EntryKind::File, 3));
    assert_eq!(seen[1].0, "sub");
    assert_eq!(seen[1].1, EntryKind::Directory);

    let meta = conn.client.stat(&rig.remote("a.txt")).expect("stat");
    assert_eq!((meta.kind, meta.len), (EntryKind::File, ByteCount(3)));
    // SHA-256("abc"), the FIPS 180-2 vector: the wire's hash is this one.
    let want =
        ContentHash::from_hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(Some(meta.stamp.hash), want);

    let cases = [(1, 10, "bc"), (0, 2, "ab"), (3, 5, ""), (99, 5, "")];
    for (offset, len, expect) in cases {
        let chunk = conn
            .client
            .read_range(&rig.remote("a.txt"), span(offset, len))
            .expect("read");
        assert_eq!(chunk.bytes, expect.as_bytes(), "{offset}+{len}");
        assert_eq!((chunk.total, chunk.stamp), (ByteCount(3), meta.stamp));
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn a_write_back_is_refused_when_the_file_changed_even_with_the_same_time() {
    let rig = Rig::new("files-write");
    let file = rig.remote("doc.txt");
    let conn = rig.connect();

    let made = conn
        .client
        .write_file(&file, WriteCheck::MustBeNew, b"one")
        .expect("a new file");
    let again = remote(
        conn.client
            .write_file(&file, WriteCheck::MustBeNew, b"two")
            .expect_err("it exists now"),
    );
    assert_eq!(
        again,
        RemoteError::Conflict {
            current: Some(made)
        }
    );

    let (seen, bytes) = conn.client.read_all(&file).expect("read");
    assert_eq!((seen, bytes.as_slice()), (made, &b"one"[..]));
    let saved = conn
        .client
        .write_back(&file, seen, b"edited")
        .expect("saved");
    assert_eq!(
        std::fs::read(rig.files.join("doc.txt")).expect("read"),
        b"edited"
    );

    // Someone else wrote since `seen`: the person's copy must not overwrite it.
    let refused = remote(
        conn.client
            .write_back(&file, seen, b"stale")
            .expect_err("changed since"),
    );
    assert_eq!(
        refused,
        RemoteError::Conflict {
            current: Some(saved)
        }
    );
    assert_eq!(
        std::fs::read(rig.files.join("doc.txt")).expect("read"),
        b"edited"
    );

    // The same time but other contents still counts as a change.
    let lookalike = FileStamp {
        hash: ContentHash([1; 32]),
        ..saved
    };
    let refused = conn
        .client
        .write_back(&file, lookalike, b"x")
        .expect_err("hash differs");
    assert!(matches!(remote(refused), RemoteError::Conflict { .. }));

    conn.client
        .write_file(&file, WriteCheck::Overwrite, b"forced")
        .expect("overwrite");
    assert_eq!(
        std::fs::read(rig.files.join("doc.txt")).expect("read"),
        b"forced"
    );
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn nothing_outside_the_granted_folders_is_reachable() {
    let rig = Rig::new("files-confine");
    std::fs::write(rig.files.join("a.txt"), b"abc").expect("file");
    std::fs::write(rig.home.join("secret"), b"s").expect("file");
    symlink("/etc", rig.files.join("etc-link")).expect("link");
    symlink(rig.files.join("a.txt"), rig.files.join("file-link")).expect("link");
    std::fs::create_dir(rig.files.join("real")).expect("folder");
    symlink(rig.home.join("secret"), rig.files.join("real").join("out")).expect("link");
    let conn = rig.connect();

    let denied = |p: RemotePath| {
        let error = conn
            .client
            .read_range(&p, span(0, 10))
            .expect_err("refused");
        remote(error)
    };
    let outside = [
        // A folder the client was never given.
        RemotePath(rig.home.join("secret").display().to_string()),
        RemotePath("/etc/passwd".to_string()),
        // A link out of the folder, whole or on the way, however deep.
        rig.remote("etc-link/passwd"),
        rig.remote("file-link"),
        rig.remote("real/out"),
    ];
    for p in outside {
        assert!(
            matches!(denied(p.clone()), RemoteError::PermissionDenied { .. }),
            "{p:?}"
        );
    }
    let listing = conn.client.list_dir(&rig.remote("etc-link"));
    assert!(matches!(
        remote(listing.expect_err("refused")),
        RemoteError::PermissionDenied { .. }
    ));

    // Paths that climb or are not absolute are not paths at all.
    for bad in [
        format!("{}/../home/secret", rig.files.display()),
        "a.txt".to_string(),
    ] {
        let error = conn
            .client
            .stat(&RemotePath(bad.clone()))
            .expect_err("refused");
        assert!(
            matches!(remote(error), RemoteError::BadRequest { .. }),
            "{bad}"
        );
    }

    // A link is reported as a link, never followed.
    let meta = conn.client.stat(&rig.remote("file-link")).expect("stat");
    assert_eq!(meta.kind, EntryKind::Symlink);

    // A session's folder is something the client asked for, so its files open up.
    if tools_present() {
        let secret = RemotePath(rig.home.join("secret").display().to_string());
        let id = conn.client.open_session(rig.bash()).expect("open");
        let chunk = conn
            .client
            .read_range(&secret, span(0, 10))
            .expect("now granted");
        assert_eq!(chunk.bytes, b"s");
        conn.client.close_session(id).expect("close");
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn watches_report_a_file_appearing_changing_and_going() {
    let rig = Rig::new("files-watch");
    let mut conn = rig.connect();
    let file = rig.remote("w.txt");
    let watch = conn.client.watch(&file).expect("watch");

    #[allow(clippy::type_complexity)] // a table of named rows
    let steps: [(&str, fn(&std::path::Path), WatchChange); 3] = [
        (
            "created",
            |p| std::fs::write(p, b"1").expect("write"),
            WatchChange::Created,
        ),
        (
            "modified",
            |p| std::fs::write(p, b"12").expect("write"),
            WatchChange::Modified,
        ),
        (
            "removed",
            |p| std::fs::remove_file(p).expect("remove"),
            WatchChange::Removed,
        ),
    ];
    for (name, act, want) in steps {
        act(&rig.files.join("w.txt"));
        rig.daemon.tick();
        let wanted = file.clone();
        conn.watch.event(name, |event| match event {
            ClientEvent::FileChanged {
                watch: w,
                path,
                kind,
            } if *w == watch && *path == wanted && *kind == want => Some(()),
            _ => None,
        });
    }

    conn.client.unwatch(watch).expect("unwatch");
    let again = conn.client.unwatch(watch).expect_err("no such watch");
    assert!(matches!(remote(again), RemoteError::NoSuchWatch { .. }));
}

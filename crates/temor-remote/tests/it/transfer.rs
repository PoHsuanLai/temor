//! Moving files through the link: chunked downloads into the downloads folder, uploads, and the
//! edit-locally round trip.

use temor_remote::Error;
use temor_transfer::{RelPath, Root};
use temor_wire::message::{RemoteError, RemotePath, WriteCheck};

use crate::support::Rig;

/// More than two chunks of a recognisable, non-repeating pattern.
fn big() -> Vec<u8> {
    (0..2_600_000u32)
        .map(|n| (n % 251) as u8 ^ (n / 4093) as u8)
        .collect()
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "receiving files (temor-transfer's confined create) is Linux-only so far"
)]
fn a_download_arrives_whole_in_the_downloads_folder_and_a_failed_one_leaves_nothing() {
    let rig = Rig::new("transfer-down");
    let data = big();
    std::fs::write(rig.files.join("big.bin"), &data).expect("file");
    let shelf = rig.home.join("shelf");
    std::fs::create_dir(&shelf).expect("folder");
    let conn = rig.connect();

    let root = Root::open(&shelf).expect("root");
    let rel = RelPath::parse("got/big.bin").expect("path");
    let remote = RemotePath(format!("{}/big.bin", rig.files.display()));
    let stamp = conn
        .client
        .download(&remote, &root, &rel)
        .expect("download");
    assert_eq!(
        std::fs::read(shelf.join("got/big.bin")).expect("read"),
        data
    );
    assert_eq!(conn.client.stat(&remote).expect("stat").stamp, stamp);

    let missing = RemotePath(format!("{}/missing.bin", rig.files.display()));
    let gone = RelPath::parse("got/missing.bin").expect("path");
    assert!(conn.client.download(&missing, &root, &gone).is_err());
    assert!(!shelf.join("got/missing.bin").exists());
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn an_upload_lands_and_the_edit_round_trip_stops_at_a_changed_file() {
    let rig = Rig::new("transfer-up");
    let conn = rig.connect();
    let local = rig.home.join("up.txt");
    std::fs::write(&local, b"from here").expect("file");
    let remote = RemotePath(format!("{}/up.txt", rig.files.display()));

    let stamp = conn
        .client
        .upload(&local, &remote, WriteCheck::MustBeNew)
        .expect("upload");
    assert_eq!(
        std::fs::read(rig.files.join("up.txt")).expect("read"),
        b"from here"
    );
    let again = conn.client.upload(&local, &remote, WriteCheck::MustBeNew);
    assert!(matches!(
        again,
        Err(Error::Remote(RemoteError::Conflict { current: Some(now) })) if now == stamp
    ));

    let (seen, bytes) = conn.client.read_all(&remote).expect("read");
    assert_eq!(bytes, b"from here");
    std::fs::write(rig.files.join("up.txt"), b"changed elsewhere").expect("other writer");
    let refused = conn.client.write_back(&remote, seen, b"my edit");
    assert!(matches!(
        refused,
        Err(Error::Remote(RemoteError::Conflict { current: Some(_) }))
    ));
    assert_eq!(
        std::fs::read(rig.files.join("up.txt")).expect("read"),
        b"changed elsewhere"
    );
}

/// The part size an upload streams in (256 KiB), as the tests see it from outside.
const PART: usize = 256 << 10;

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn an_upload_reports_every_byte_once_and_ends_at_the_files_size() {
    let rig = Rig::new("transfer-progress");
    let conn = rig.connect();
    let cases: [(&str, usize, &[u64]); 5] = [
        ("empty", 0, &[0]),
        ("small", 10, &[10]),
        ("one part", PART, &[PART as u64]),
        ("one over", PART + 1, &[PART as u64, 1]),
        ("three", 2 * PART + 5, &[PART as u64, PART as u64, 5]),
    ];
    for (name, size, parts) in cases {
        let local = rig.home.join(format!("{name}.bin"));
        let data: Vec<u8> = (0..size).map(|n| (n % 251) as u8).collect();
        std::fs::write(&local, &data).expect("file");
        let remote = RemotePath(format!("{}/{name}.bin", rig.files.display()));
        let mut seen = Vec::new();
        conn.client
            .upload_with(&local, &remote, WriteCheck::MustBeNew, &mut |n| {
                seen.push(n)
            })
            .expect("upload");
        assert_eq!(seen, parts, "{name}: the parts the hook saw");
        assert_eq!(seen.iter().sum::<u64>(), size as u64, "{name}: the total");
    }
}

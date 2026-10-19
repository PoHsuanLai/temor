//! Protocol version 2: ping, the window's colours, and streamed writes.

use std::time::Duration;

use temor_remote::{DAEMON_CAPS, Error, hello};
use temor_wire::hello::Capability;
use temor_wire::message::{Palette, PartEnd, RemoteError, RemotePath, Rgb, WriteCheck};
use temor_wire::units::ByteOffset;

use crate::support::{HELPER, Rig, tools_present};

const SOON: Duration = Duration::from_secs(10);

#[test]
fn a_ping_is_answered_whatever_was_agreed() {
    let rig = Rig::new("ping");
    for (name, caps) in [
        ("all", DAEMON_CAPS.to_vec()),
        ("none", Vec::<Capability>::new()),
    ] {
        let conn = rig.connect_with(hello(HELPER, &caps));
        assert!(conn.client.ping(SOON).is_ok(), "{name}");
    }
}

fn palette() -> Palette {
    let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    ansi[1] = Rgb {
        r: 0xaa,
        g: 0,
        b: 0,
    };
    Palette {
        ansi,
        foreground: Rgb {
            r: 0xde,
            g: 0xad,
            b: 0xbe,
        },
        background: Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        },
        cursor: Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        },
    }
}

#[test]
fn a_program_that_asks_for_the_background_gets_the_windows() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("palette");
    let mut conn = rig.connect();
    let id = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(id).expect("attach");
    conn.client.set_palette(id, palette()).expect("palette");
    // Ask the way a program does (OSC 11), reading the answer raw from the terminal.
    let script = concat!(
        "stty raw -echo; printf '\\033]11;?\\007'; ",
        "IFS= read -r -d $'\\a' -t 5 reply; stty sane; ",
        "echo \"ANSWER=${reply//[^[:print:]]/.}\"\n"
    );
    conn.client
        .send_input(id, script.as_bytes())
        .expect("input");
    conn.watch
        .output_has("the answer", |out| out.contains("rgb:1212/3434/5656"));
}

fn part(
    conn: &crate::support::Conn,
    path: &RemotePath,
    offset: u64,
    end: PartEnd,
    bytes: &[u8],
) -> Result<Option<temor_wire::message::FileStamp>, Error> {
    conn.client
        .write_part(path, WriteCheck::MustBeNew, ByteOffset(offset), end, bytes)
}

fn staged_files(rig: &Rig) -> Vec<String> {
    std::fs::read_dir(&rig.files)
        .expect("folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with(".temor-part-"))
        .collect()
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn parts_land_once_at_the_end_and_a_bad_part_or_a_refused_check_leaves_nothing() {
    let rig = Rig::new("parts");
    let conn = rig.connect();
    let path = RemotePath(format!("{}/streamed.bin", rig.files.display()));
    let target = rig.files.join("streamed.bin");

    // In order: nothing shows until the last part, then the whole file does.
    assert_eq!(
        part(&conn, &path, 0, PartEnd::More, b"abc").expect("first"),
        None
    );
    assert_eq!(staged_files(&rig).len(), 1);
    assert!(!target.exists());
    assert_eq!(
        part(&conn, &path, 3, PartEnd::More, b"def").expect("second"),
        None
    );
    let stamp = part(&conn, &path, 6, PartEnd::Last, b"g")
        .expect("last")
        .expect("a stamp");
    assert_eq!(std::fs::read(&target).expect("read"), b"abcdefg");
    assert_eq!(conn.client.stat(&path).expect("stat").stamp, stamp);
    assert!(staged_files(&rig).is_empty());

    // A part that skips ahead is refused and does not disturb what was staged.
    part(&conn, &path, 0, PartEnd::More, b"xy").expect("restart");
    let skipped = part(&conn, &path, 9, PartEnd::More, b"z");
    assert!(matches!(
        skipped,
        Err(Error::Remote(RemoteError::BadRequest { .. }))
    ));
    // The file exists now, so MustBeNew at the end is a conflict, and the staged bytes go.
    let refused = part(&conn, &path, 2, PartEnd::Last, b"!");
    assert!(matches!(
        refused,
        Err(Error::Remote(RemoteError::Conflict { .. }))
    ));
    assert_eq!(std::fs::read(&target).expect("read"), b"abcdefg");
    assert!(staged_files(&rig).is_empty());

    // Abort drops what was staged; a part with nothing staged before it is refused.
    part(&conn, &path, 0, PartEnd::More, b"q").expect("restart");
    assert_eq!(
        part(&conn, &path, 1, PartEnd::Abort, b"").expect("abort"),
        None
    );
    assert!(staged_files(&rig).is_empty());
    let orphan = part(&conn, &path, 4, PartEnd::More, b"q");
    assert!(matches!(
        orphan,
        Err(Error::Remote(RemoteError::BadRequest { .. }))
    ));
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn an_upload_bigger_than_one_part_streams() {
    let rig = Rig::new("upload-big");
    let conn = rig.connect();
    // Two and a bit parts of a pattern that shows a part landing in the wrong place.
    let data: Vec<u8> = (0..9_500_000u32)
        .map(|n| (n % 251) as u8 ^ (n / 4093) as u8)
        .collect();
    let local = rig.home.join("big.bin");
    std::fs::write(&local, &data).expect("file");
    let remote = RemotePath(format!("{}/big.bin", rig.files.display()));

    let stamp = conn
        .client
        .upload(&local, &remote, WriteCheck::MustBeNew)
        .expect("upload");
    assert_eq!(
        std::fs::read(rig.files.join("big.bin")).expect("read"),
        data
    );
    assert_eq!(conn.client.stat(&remote).expect("stat").stamp, stamp);
    assert!(staged_files(&rig).is_empty());

    // The same again is refused by the check at the end, and the file is as it was.
    let again = conn.client.upload(&local, &remote, WriteCheck::MustBeNew);
    assert!(matches!(
        again,
        Err(Error::Remote(RemoteError::Conflict { current: Some(now) })) if now == stamp
    ));
    assert!(staged_files(&rig).is_empty());
}

//! Staged parts of uploads that never finished: the daemon clears the old ones when it starts,
//! and a streamed upload that is refused leaves none behind.

use std::fs::File;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use temor_remote::Error;
use temor_remote::files::{STAGE_PREFIX, stage_name};
use temor_wire::message::{RemoteError, RemotePath, WriteCheck};
use temor_wire::units::UnixMillis;

use crate::support::Rig;

fn now_millis() -> UnixMillis {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970");
    UnixMillis(i64::try_from(since.as_millis()).expect("fits"))
}

fn staged(dir: &std::path::Path, leaf: &str, age: Duration) {
    let path = dir.join(stage_name(leaf));
    std::fs::write(&path, b"half").expect("staged file");
    let file = File::options().write(true).open(&path).expect("open");
    file.set_modified(SystemTime::now() - age).expect("age it");
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn starting_clears_old_staged_parts_and_nothing_else() {
    let rig = Rig::new("sweep-start");
    let day = Duration::from_secs(24 * 60 * 60);
    std::fs::create_dir_all(rig.files.join("deep/er")).expect("folders");
    staged(&rig.files, "old.bin", day * 2);
    staged(&rig.files, "recent.bin", Duration::from_secs(3600));
    staged(&rig.files.join("deep/er"), "buried.bin", day * 3);
    std::fs::write(rig.files.join("notes.txt"), b"mine").expect("file");
    // A hidden file of the person's own, which is not a staged part.
    std::fs::write(rig.files.join(".keep"), b"").expect("file");

    let removed = rig.daemon.sweep_staged(now_millis());

    assert_eq!(removed, 2);
    let left = names(&rig.files);
    assert!(left.contains(&stage_name("recent.bin")), "{left:?}");
    assert!(!left.contains(&stage_name("old.bin")), "{left:?}");
    assert!(left.contains(&"notes.txt".to_string()) && left.contains(&".keep".to_string()));
    assert!(names(&rig.files.join("deep/er")).is_empty());
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn an_upload_over_a_changed_file_is_refused_and_leaves_no_staged_part() {
    let rig = Rig::new("sweep-refused");
    let conn = rig.connect();
    let target = rig.files.join("big.bin");
    std::fs::write(&target, b"already here").expect("file");
    // Bigger than one part, so it streams.
    let local = rig.home.join("big-up.bin");
    std::fs::write(&local, vec![7u8; 5 << 20]).expect("file");
    let remote = RemotePath(target.display().to_string());

    let refused = conn.client.upload(&local, &remote, WriteCheck::MustBeNew);

    assert!(matches!(
        refused,
        Err(Error::Remote(RemoteError::Conflict { current: Some(_) }))
    ));
    assert_eq!(std::fs::read(&target).expect("read"), b"already here");
    let strays: Vec<String> = names(&rig.files)
        .into_iter()
        .filter(|name| name.starts_with(STAGE_PREFIX))
        .collect();
    assert!(strays.is_empty(), "{strays:?}");
}

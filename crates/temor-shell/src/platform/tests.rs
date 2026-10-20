use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

use super::ensure_private_dir;
use crate::error::ShellError;

fn mode_of(path: &std::path::Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[test]
fn a_missing_folder_is_made_0700() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("private");
    ensure_private_dir(&dir).expect("made");
    assert_eq!(mode_of(&dir), 0o700);
}

#[test]
fn a_folder_of_ours_in_any_mode_is_tightened_not_refused() {
    // 0300 and 0000 cannot be opened for reading; 0500 and 0755 can, and are not 0700 either.
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("private");
    ensure_private_dir(&dir).expect("made");
    for mode in [0o300, 0o000, 0o500, 0o755] {
        fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).expect("chmod");
        ensure_private_dir(&dir).unwrap_or_else(|err| panic!("{mode:o}: {err}"));
        assert_eq!(mode_of(&dir), 0o700, "from {mode:o}");
    }
}

#[test]
fn a_link_or_a_file_in_its_place_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let real = root.path().join("real");
    fs::create_dir(&real).expect("real");
    let link = root.path().join("link");
    symlink(&real, &link).expect("link");
    let file = root.path().join("file");
    fs::write(&file, "").expect("file");
    for path in [link, file] {
        assert!(
            matches!(ensure_private_dir(&path), Err(ShellError::PrivateDir { .. })),
            "{path:?}"
        );
    }
}

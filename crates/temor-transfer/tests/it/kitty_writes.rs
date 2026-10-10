//! The platform edge: writes resolve from a root descriptor, so links on disk, and links planted
//! between two writes, are refused by the kernel.
#![cfg(target_os = "linux")]

use std::os::unix::fs::symlink;

use temor_transfer::kitty::{Mode, WriteOp, apply_write};
use temor_transfer::{RelPath, Root, TransferError};

fn rel(raw: &str) -> RelPath {
    RelPath::parse(raw).expect("path")
}

#[test]
fn applying_writes_creates_files_under_the_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = Root::open(dir.path()).expect("root");
    apply_write(&root, &WriteOp::MakeDir(rel("sub"))).expect("dir");
    apply_write(&root, &WriteOp::CreateFile(rel("sub/out.txt"))).expect("create");
    apply_write(
        &root,
        &WriteOp::Append {
            path: rel("sub/out.txt"),
            data: b"hi".to_vec(),
        },
    )
    .expect("append");
    apply_write(
        &root,
        &WriteOp::Finish {
            path: rel("sub/out.txt"),
            modified: None,
            mode: None,
        },
    )
    .expect("finish");
    assert_eq!(
        std::fs::read(dir.path().join("sub/out.txt")).expect("read"),
        b"hi"
    );
    assert!(matches!(
        apply_write(&root, &WriteOp::CreateFile(rel("sub/out.txt"))),
        Err(TransferError::Io { .. })
    ));
}

#[test]
fn a_symlink_planted_between_create_and_append_is_refused() {
    let root_dir = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let root = Root::open(root_dir.path()).expect("root");
    apply_write(&root, &WriteOp::CreateFile(rel("a.txt"))).expect("create");
    std::fs::remove_file(root_dir.path().join("a.txt")).expect("remove");
    let target = outside.path().join("victim.txt");
    std::fs::write(&target, b"").expect("victim");
    symlink(&target, root_dir.path().join("a.txt")).expect("plant");
    let result = apply_write(
        &root,
        &WriteOp::Append {
            path: rel("a.txt"),
            data: b"pwned".to_vec(),
        },
    );
    assert!(
        matches!(result, Err(TransferError::UnsafePath { .. })),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&target).expect("read"), b"");
}

#[test]
fn a_folder_swapped_for_a_link_between_writes_is_refused() {
    let root_dir = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let root = Root::open(root_dir.path()).expect("root");
    apply_write(&root, &WriteOp::MakeDir(rel("sub"))).expect("dir");
    std::fs::remove_dir(root_dir.path().join("sub")).expect("remove");
    symlink(outside.path(), root_dir.path().join("sub")).expect("plant");
    let result = apply_write(&root, &WriteOp::CreateFile(rel("sub/x.txt")));
    assert!(
        matches!(result, Err(TransferError::UnsafePath { .. })),
        "{result:?}"
    );
    assert!(!outside.path().join("x.txt").exists());
}

#[test]
fn folders_cannot_be_made_through_a_link_already_on_disk() {
    let root_dir = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    symlink(outside.path(), root_dir.path().join("link")).expect("symlink");
    let root = Root::open(root_dir.path()).expect("root");
    let result = apply_write(&root, &WriteOp::MakeDir(rel("link/newdir")));
    assert!(
        matches!(result, Err(TransferError::UnsafePath { .. })),
        "{result:?}"
    );
    assert!(!outside.path().join("newdir").exists());
}

#[test]
fn discarding_a_link_removes_the_link_and_not_its_target() {
    let root_dir = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let keep = outside.path().join("keep.txt");
    std::fs::write(&keep, b"keep").expect("keep");
    symlink(&keep, root_dir.path().join("link")).expect("symlink");
    let root = Root::open(root_dir.path()).expect("root");
    apply_write(&root, &WriteOp::Discard(rel("link"))).expect("discard");
    assert!(!root_dir.path().join("link").exists());
    assert_eq!(std::fs::read(&keep).expect("read"), b"keep");
}

#[test]
fn a_remote_mode_is_applied_without_setuid_and_a_folder_keeps_the_owner_in() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let root = Root::open(dir.path()).expect("root");
    apply_write(&root, &WriteOp::MakeDir(rel("sub"))).expect("dir");
    let sealed = WriteOp::SetDirMode {
        path: rel("sub"),
        mode: Mode::new(0o040),
    };
    apply_write(&root, &sealed).expect("dir mode");
    apply_write(&root, &WriteOp::CreateFile(rel("sub/run.sh"))).expect("create");
    let finish = WriteOp::Finish {
        path: rel("sub/run.sh"),
        modified: None,
        mode: Some(Mode::new(0o4755)),
    };
    apply_write(&root, &finish).expect("finish");
    let mode_of = |p: &str| {
        std::fs::metadata(dir.path().join(p))
            .expect("meta")
            .permissions()
            .mode()
            & 0o7777
    };
    assert_eq!(mode_of("sub/run.sh"), 0o755, "setuid dropped");
    assert_eq!(mode_of("sub"), 0o740, "the owner keeps rwx");
}

#[test]
fn a_link_is_made_inside_the_root_and_never_over_a_name_that_is_taken() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = Root::open(dir.path()).expect("root");
    let link = |path: &str, target: &str| WriteOp::Symlink {
        path: rel(path),
        target: target.to_owned(),
    };
    apply_write(&root, &link("sub/l", "../a.txt")).expect("link");
    assert_eq!(
        std::fs::read_link(dir.path().join("sub/l")).expect("read link"),
        std::path::PathBuf::from("../a.txt")
    );
    assert!(apply_write(&root, &link("sub/l", "b")).is_err());
    // Writing through the link is still refused.
    let through = apply_write(&root, &WriteOp::CreateFile(rel("sub/l/x")));
    assert!(
        matches!(through, Err(TransferError::UnsafePath { .. })),
        "{through:?}"
    );
}

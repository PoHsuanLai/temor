//! The platform edge: writes resolve from a root descriptor, so links on disk, and links planted
//! between two writes, are refused by the kernel.
#![cfg(target_os = "linux")]

use std::os::unix::fs::symlink;

use temor_transfer::kitty::{WriteOp, apply_write};
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

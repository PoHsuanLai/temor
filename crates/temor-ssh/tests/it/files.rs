//! Remote files through stand-in `ssh` and `scp` programs: the fake `ssh` runs the remote command
//! on this machine, so the stat script really runs under `sh` with an awkward path, and the fake
//! `scp` copies between the two paths. Nothing connects anywhere.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use temor_ssh::{
    ByteCount, ControlLayout, ControlPersist, FileTools, HostAlias, RemoteFilePath, RemoteFiles,
    RemoteStat, RemoteState, RuntimeDir, SshRemoteFiles, UnixSeconds,
};

use crate::harness::{RetryRunner, make_bin, scratch, write};

/// Runs the last argument (the remote command) with `sh -c`, as the host's shell would.
const FAKE_SSH: &str = "#!/bin/sh\nfor last; do :; done\nexec sh -c \"$last\"\n";

/// Copies the two arguments after `--`, dropping the `fake:` host prefix.
const FAKE_SCP: &str = "#!/bin/sh\n\
seen=no; src=; dst=\n\
for a; do\n\
  if [ \"$a\" = -- ]; then seen=yes; continue; fi\n\
  [ $seen = yes ] || continue\n\
  case $a in fake:*) a=${a#fake:} ;; esac\n\
  if [ -z \"$src\" ]; then src=$a; else dst=$a; fi\n\
done\n\
exec cp -- \"$src\" \"$dst\"\n";

fn files(bin: &Path) -> SshRemoteFiles<RetryRunner> {
    let layout = ControlLayout::new(
        RuntimeDir::new(Path::new("/run/user/1000/temor")).expect("runtime"),
        ControlPersist::Forever,
    )
    .expect("layout");
    let tools = FileTools {
        ssh: bin.join("ssh"),
        scp: bin.join("scp"),
    };
    SshRemoteFiles::new(RetryRunner, tools, layout)
}

#[test]
fn an_awkward_path_is_fetched_edited_and_written_back() {
    let s = scratch();
    make_bin(
        &s.bin,
        &["sh", "cp", "stat", "cat"],
        &[("ssh", FAKE_SSH), ("scp", FAKE_SCP)],
    );
    let remote = s.work.join("it's a $HOME \"file\" (1).txt");
    write(&remote, "remote v1\n");
    let remote_path = RemoteFilePath::new(&remote.display().to_string()).expect("remote path");
    let host = HostAlias::new("fake").expect("alias");
    let files = files(&s.bin);

    let meta = fs::metadata(&remote).expect("metadata");
    let want = RemoteStat {
        mtime: UnixSeconds(u64::try_from(meta.mtime()).expect("positive mtime")),
        size: ByteCount(meta.len()),
    };
    let local = s.tmp.join("copy.txt");
    let opened = files
        .download(&host, &remote_path, &local)
        .expect("download");
    assert_eq!(opened, Some(want));
    assert_eq!(
        fs::read_to_string(&local).expect("local copy"),
        "remote v1\n"
    );

    write(&local, "edited locally\n");
    files.store(&host, &local, &remote_path).expect("store");
    assert_eq!(
        fs::read_to_string(&remote).expect("remote"),
        "edited locally\n"
    );
    let after = files.stat(&host, &remote_path).expect("stat");
    assert!(
        matches!(after, RemoteState::Present(stat) if stat.size == ByteCount(15)),
        "{after:?}"
    );

    fs::remove_file(&remote).expect("remove");
    assert_eq!(
        files.stat(&host, &remote_path).expect("stat"),
        RemoteState::Missing
    );
}

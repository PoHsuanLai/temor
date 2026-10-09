use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    ByteCount, CommandOutput, CommandRunner, FileTools, RemoteFilePath, RemoteFiles, RemoteStat,
    RemoteState, SshRemoteFiles, fetch_argv, parse_stat, stat_argv, store_argv,
};
use crate::argv::{ControlLayout, ControlPersist, RuntimeDir};
use crate::config::HostAlias;
use crate::error::SshError;
use crate::tailnet::UnixSeconds;

fn layout() -> ControlLayout {
    ControlLayout::new(
        RuntimeDir::new(Path::new("/run/user/1000/temor")).expect("runtime"),
        ControlPersist::Forever,
    )
    .expect("layout")
}

fn host() -> HostAlias {
    HostAlias::new("prod").expect("alias")
}

fn path(text: &str) -> RemoteFilePath {
    RemoteFilePath::new(text).expect("path")
}

/// A runner that records each call and answers from a script.
struct Fake {
    calls: RefCell<Vec<(PathBuf, Vec<String>)>>,
    answers: RefCell<VecDeque<io::Result<CommandOutput>>>,
}

impl Fake {
    fn new(answers: Vec<io::Result<CommandOutput>>) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            answers: RefCell::new(answers.into()),
        }
    }
}

impl CommandRunner for &Fake {
    fn run(&self, program: &Path, args: &[String]) -> io::Result<CommandOutput> {
        self.calls
            .borrow_mut()
            .push((program.to_path_buf(), args.to_vec()));
        self.answers
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(io::Error::other("no answer scripted")))
    }
}

fn ok(stdout: &str) -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        code: Some(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    })
}

#[test]
fn stat_output_is_read_from_its_last_line() {
    let present = |m, s| {
        Some(RemoteState::Present(RemoteStat {
            mtime: UnixSeconds(m),
            size: ByteCount(s),
        }))
    };
    // Name, output, result (`None` for an answer that is not understood).
    let cases = [
        ("plain", "1700000000 42\n", present(1_700_000_000, 42)),
        (
            "after a banner",
            "Welcome\n\n1700000000 0\n",
            present(1_700_000_000, 0),
        ),
        ("gone", "missing\n", Some(RemoteState::Missing)),
        ("empty", "", None),
        ("one number", "1700000000\n", None),
        ("three numbers", "1 2 3\n", None),
        ("not numbers", "x y\n", None),
        ("a negative time", "-5 3\n", None),
    ];
    for (name, output, want) in cases {
        assert_eq!(parse_stat(output).ok(), want, "{name}");
    }
}

#[test]
fn remote_paths_are_absolute_and_free_of_wildcards_and_control_characters() {
    for good in ["/etc/hosts", "/home/me/it's a $file.txt", "/a b/c"] {
        assert!(RemoteFilePath::new(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "relative/x",
        "~/x",
        "/a/*.txt",
        "/a?",
        "/a[1]",
        "/a{b,c}",
        "/a\\b",
        "/a\nb",
        "/a\u{7}",
    ] {
        assert!(
            matches!(RemoteFilePath::new(bad), Err(SshError::BadRemotePath(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn file_commands_join_the_master_and_never_start_a_connection() {
    let fetch = fetch_argv(
        &host(),
        &layout(),
        &path("/srv/a b.txt"),
        Path::new("/tmp/x/a b.txt"),
    )
    .expect("fetch");
    let store = store_argv(
        &host(),
        &layout(),
        Path::new("/tmp/x/a b.txt"),
        &path("/srv/a b.txt"),
    )
    .expect("store");
    let stat = stat_argv(&host(), &layout(), &path("/srv/a b.txt")).expect("stat");
    for (name, argv) in [("fetch", &fetch), ("store", &store), ("stat", &stat)] {
        for needed in [
            "BatchMode=yes",
            "ControlMaster=no",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "--",
        ] {
            assert!(
                argv.iter().any(|a| a == needed),
                "{name} lacks {needed}: {argv:?}"
            );
        }
    }
    assert_eq!(
        fetch[fetch.len() - 2..],
        ["prod:/srv/a b.txt", "/tmp/x/a b.txt"]
    );
    assert_eq!(
        store[store.len() - 2..],
        ["/tmp/x/a b.txt", "prod:/srv/a b.txt"]
    );
    assert!(
        fetch.contains(&"-s".to_owned()),
        "the file-transfer protocol, not a remote shell"
    );
    // The path reaches the host inside the encoded script, never as a bare argument.
    assert!(!stat.iter().any(|a| a.contains("a b.txt")), "{stat:?}");
    assert!(stat.last().is_some_and(|a| a.starts_with("sh -c '")));
}

#[test]
fn local_paths_must_be_absolute_so_a_colon_cannot_make_a_host_of_them() {
    for bad in ["rel/x", "host:x", "-rf", "/tmp/a\nb"] {
        let got = fetch_argv(&host(), &layout(), &path("/x"), Path::new(bad));
        assert!(matches!(got, Err(SshError::BadLocalPath(_))), "{bad:?}");
    }
}

#[test]
fn download_reads_the_stat_before_the_content_and_a_gone_file_is_none() {
    let fake = Fake::new(vec![ok("1700000000 5\n"), ok(""), ok("missing\n")]);
    let files = SshRemoteFiles::new(&fake, FileTools::default(), layout());
    let got = files
        .download(&host(), &path("/srv/a"), Path::new("/tmp/a"))
        .expect("download");
    assert_eq!(
        got,
        Some(RemoteStat {
            mtime: UnixSeconds(1_700_000_000),
            size: ByteCount(5)
        })
    );
    let programs: Vec<_> = fake.calls.borrow().iter().map(|(p, _)| p.clone()).collect();
    assert_eq!(programs, [PathBuf::from("ssh"), PathBuf::from("scp")]);
    let gone = files
        .download(&host(), &path("/srv/b"), Path::new("/tmp/b"))
        .expect("download");
    assert_eq!(gone, None);
    assert_eq!(fake.calls.borrow().len(), 3, "a gone file is not fetched");
}

#[test]
fn a_failing_program_says_what_it_printed_and_a_missing_program_says_which() {
    let refused = Ok(CommandOutput {
        code: Some(255),
        stdout: Vec::new(),
        stderr: b"\nControl socket connect(/run/x): No such file\x07 or directory\nsecond\n"
            .to_vec(),
    });
    let fake = Fake::new(vec![refused, Err(io::Error::from(io::ErrorKind::NotFound))]);
    let files = SshRemoteFiles::new(&fake, FileTools::default(), layout());
    let err = files.stat(&host(), &path("/x")).expect_err("fails");
    assert!(
        matches!(
            &err,
            SshError::RemoteFile { code: Some(255), detail, .. }
                if detail == "Control socket connect(/run/x): No such file or directory"
        ),
        "{err:?}"
    );
    let err = files
        .store(&host(), Path::new("/tmp/a"), &path("/x"))
        .expect_err("fails");
    assert!(
        matches!(&err, SshError::Spawn { program, .. } if program == "scp"),
        "{err:?}"
    );
}

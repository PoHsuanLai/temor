//! kitty's own `kitten transfer` sending a file into temor's receive machine: the kitten runs
//! under python's `pty.spawn`, which gives it a terminal, and what it writes there is answered as temor
//! answers it. (Its pty has no size, so the kitten panics on a divide by zero once the file is
//! across; that is the harness, not the transfer.) Run by hand (it is ignored): `cargo test -p temor-transfer --test it
//! kitty_interop -- --ignored` with kitten on PATH. Skipped where kitten or python3 is missing.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use temor_transfer::kitty::Link;

use crate::roundtrip::terminal_takes;

fn tool(name: &str) -> Option<PathBuf> {
    let found = std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file());
    if found.is_none() {
        assert!(
            std::env::var("TEMOR_REQUIRE_TOOLS").as_deref() != Ok("1"),
            "{name} is required (TEMOR_REQUIRE_TOOLS=1) but is not on PATH"
        );
        eprintln!("skipping: {name} is not installed");
    }
    found
}

/// The kitten's terminal, seen from the terminal's side.
struct Kitten {
    child: Child,
    stdin: ChildStdin,
    out: Receiver<Vec<u8>>,
}

impl Kitten {
    /// `kitten transfer <src> <dest>` in `cwd`, which is also its HOME.
    fn start(kitten: &Path, python: &Path, src: &str, dest: &str, cwd: &Path) -> Self {
        let mut child = Command::new(python)
            .args(["-I", "-c", "import pty, sys; pty.spawn(sys.argv[1:])"])
            .arg(kitten)
            .args(["transfer", src, dest])
            .current_dir(cwd)
            .env("TERM", "xterm-kitty")
            .env("HOME", cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("script");
        let stdin = child.stdin.take().expect("stdin");
        let mut stdout = child.stdout.take().expect("stdout");
        let (tx, out) = channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 65536];
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, out }
    }
}

impl Link for Kitten {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stdin.write_all(bytes)?;
        self.stdin.flush()
    }

    fn read(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        match self.out.recv_timeout(timeout) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(io::ErrorKind::UnexpectedEof.into()),
        }
    }
}

impl Drop for Kitten {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "runs a downloaded kitten: by hand, with kitten on PATH, `-- --ignored`"]
fn kittys_own_kitten_sends_a_file_into_temor() {
    let (Some(kitten), Some(python)) = (tool("kitten"), tool("python3")) else {
        return;
    };
    let from = tempfile::tempdir().expect("from");
    let into = tempfile::tempdir().expect("into");
    std::fs::write(from.path().join("notes.txt"), b"sent by the real kitten\n").expect("file");
    let link = Kitten::start(&kitten, &python, "notes.txt", "notes.txt", from.path());
    let terminal = terminal_takes(link, into.path().to_path_buf());
    terminal.join().expect("the terminal side");
    assert_eq!(
        std::fs::read(into.path().join("notes.txt")).expect("received"),
        b"sent by the real kitten\n"
    );
}

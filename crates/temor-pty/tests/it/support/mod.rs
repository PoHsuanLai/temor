//! Shared helpers: scratch directories, launching, and reading a terminal until a pattern.

mod plain;
mod term;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use temor_pty::{
    Cols, EnvContract, GridSize, PaneId, ProgramVersion, Rows, SessionLauncher, TermName,
    child_env, find_program,
};
use temor_shell::ChildEnv;
use tempfile::TempDir;

pub use plain::{bash, bash_interactive, proc_alive};
pub use term::{Term, find};

/// A scratch HOME, runtime directory and working directory, removed when dropped.
pub struct Scratch {
    _root: TempDir,
    pub home: PathBuf,
    pub runtime: PathBuf,
    pub cwd: PathBuf,
}

impl Scratch {
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("pty-it-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .expect("scratch dir");
        let home = root.path().join("home");
        let runtime = root.path().join("run");
        let cwd = root.path().join("work dir é");
        for dir in [&home, &runtime, &cwd] {
            std::fs::create_dir_all(dir).expect("scratch subdir");
        }
        Self {
            _root: root,
            home,
            runtime,
            cwd,
        }
    }

    /// A minimal complete environment: the test's PATH, the scratch HOME, a UTF-8 locale.
    pub fn base_env(&self) -> ChildEnv {
        let mut env = ChildEnv::new();
        env.set("PATH", test_path());
        env.set("HOME", &self.home);
        env.set("LANG", "C.UTF-8");
        env
    }

    /// The base environment with temor's contract on top.
    pub fn contract_env(&self, profile: &ChildEnv) -> ChildEnv {
        let contract = EnvContract {
            term: TermName::Xterm256Color,
            version: ProgramVersion("0.0.0-test".into()),
            pane: PaneId("pane-1".into()),
            socket: None,
        };
        child_env(self.base_env(), profile, &contract)
    }
}

fn test_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into())
}

/// The program `name` on the test's PATH, if installed.
pub fn program(name: &str) -> Option<PathBuf> {
    find_program(name, &test_path())
}

/// The program `name`, or a printed note that the test is skipped. With `TEMOR_REQUIRE_TOOLS=1`
/// (the gate sets it) a missing program fails the test instead, so a gate machine without zsh,
/// fish or nu cannot pass these tests with no assertions.
pub fn program_or_skip(name: &str, test: &str) -> Option<PathBuf> {
    let found = program(name);
    if found.is_none() {
        assert!(
            std::env::var("TEMOR_REQUIRE_TOOLS").as_deref() != Ok("1"),
            "{name} is required (TEMOR_REQUIRE_TOOLS=1) but is not on PATH ({test})"
        );
        println!("note: {test} skipped: {name} is not installed");
    }
    found
}

/// util-linux `setsid` where installed (Linux), else this crate's trampoline.
pub fn setsid() -> SessionLauncher {
    program("setsid").map_or_else(trampoline, SessionLauncher::Setsid)
}

/// Whether this platform has no terminal yet; if so, prints a skip note for `test`.
pub fn pty_unsupported(test: &str) -> bool {
    let spec = temor_pty::SpawnSpec {
        program: env!("CARGO_BIN_EXE_temor-pty-exec").into(),
        args: Vec::new(),
        cwd: std::env::temp_dir(),
        env: ChildEnv::new(),
        size: size(),
    };
    match temor_pty::Pty::spawn(&spec, &trampoline()) {
        Err(temor_pty::PtyError::Unsupported) => {
            println!("note: {test} skipped: no terminal support on this platform yet");
            true
        }
        Ok(mut pty) => {
            // The trampoline with no program exits at once; reap it.
            let _ = pty.wait_exit();
            false
        }
        Err(_) => false,
    }
}

/// This crate's trampoline, the portable launcher.
pub fn trampoline() -> SessionLauncher {
    SessionLauncher::Trampoline {
        program: env!("CARGO_BIN_EXE_temor-pty-exec").into(),
        args: Vec::new(),
    }
}

pub fn size() -> GridSize {
    GridSize::cells(Cols(80), Rows(24))
}

/// `path` percent-encoded the way the integration scripts encode it.
pub fn url_path(path: &Path) -> String {
    path.as_os_str()
        .as_encoded_bytes()
        .iter()
        .map(|&b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'~' | b'-' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

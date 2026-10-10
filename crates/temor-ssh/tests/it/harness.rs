//! Scratch directories, fake tools on a private PATH, and running the bootstrap.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use temor_shell::integration_scripts;
use temor_ssh::{
    BootstrapScript, CommandOutput, CommandRunner, RemoteCommand, StdRunner,
    bootstrap_remote_command,
};

/// One test's directories. Dropped with the temp root.
pub struct Scratch {
    _root: tempfile::TempDir,
    pub home: PathBuf,
    pub tmp: PathBuf,
    pub work: PathBuf,
    pub bin: PathBuf,
}

pub fn scratch() -> Scratch {
    let root = tempfile::tempdir().expect("temp root");
    let base = root.path().to_path_buf();
    let s = Scratch {
        home: base.join("home"),
        tmp: base.join("tmp"),
        work: base.join("work"),
        bin: base.join("bin"),
        _root: root,
    };
    for dir in [&s.home, &s.tmp, &s.work, &s.bin] {
        fs::create_dir_all(dir).expect("scratch dir");
    }
    s
}

/// The absolute path of `name` on the test process's PATH.
pub fn tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Puts symlinks to `tools` (those that exist) and executable `fakes` into `dir`.
pub fn make_bin(dir: &Path, tools: &[&str], fakes: &[(&str, &str)]) {
    for name in tools {
        if let Some(real) = tool(name) {
            symlink(real, dir.join(name)).expect("symlink tool");
        }
    }
    for (name, body) in fakes {
        let path = dir.join(name);
        fs::write(&path, body).expect("fake tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

pub fn write(path: &Path, text: &str) {
    fs::write(path, text).expect("write fixture");
}

/// Every file under `root` with its contents, relative paths, sorted. Skips `.terminfo`, which
/// the bootstrap may create on hosts without Ghostty's entry.
pub fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    collect(root, root, &mut out);
    out.sort();
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".terminfo") {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push((rel, fs::read(&path).unwrap_or_default()));
        }
    }
}

/// The bootstrap as the remote command, built from the real integration scripts.
pub fn remote() -> RemoteCommand {
    let script = BootstrapScript::build(&integration_scripts()).expect("bootstrap");
    bootstrap_remote_command(&script).expect("fits in one exec argument")
}

pub struct Ran {
    pub status: ExitStatus,
    /// stdout and stderr together, since the shell prompts on stderr.
    pub text: Vec<u8>,
}

/// Whether TEMOR_REQUIRE_TOOLS=1 is set, which makes a missing shell a failure, not a skip.
fn required() -> bool {
    std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1")
}

/// The tool on PATH, or `None` after a printed note. With TEMOR_REQUIRE_TOOLS=1 a missing
/// tool fails the test instead.
pub fn require(name: &str) -> Option<PathBuf> {
    let found = tool(name);
    if found.is_none() {
        assert!(
            !required(),
            "{name} is required (TEMOR_REQUIRE_TOOLS=1) but is not on PATH"
        );
        eprintln!("skipping {name}: not installed on this machine");
    }
    found
}

/// One run of the bootstrap: `outer -c <command>`, where the outer login shell runs the remote
/// command with a scratch environment. Every field is what the bootstrap sees.
pub struct Launch {
    pub outer: PathBuf,
    pub command: String,
    pub path: String,
    /// The person's login shell, `$SHELL`, which the bootstrap starts.
    pub shell: String,
    pub home: PathBuf,
    pub tmp: PathBuf,
    pub cwd: PathBuf,
    /// What the person types into the login shell.
    pub input: String,
}

impl Launch {
    /// The bootstrap run by the system `sh`, with the system PATH, a scratch HOME and TMPDIR,
    /// the same `sh` as the login shell, and `exit` as the input.
    pub fn new(s: &Scratch) -> Self {
        let sh = tool("sh").expect("sh on the test PATH");
        Self {
            outer: sh.clone(),
            command: remote().as_str().to_owned(),
            path: system_path(),
            shell: sh.display().to_string(),
            home: s.home.clone(),
            tmp: s.tmp.clone(),
            cwd: s.work.clone(),
            input: "exit\n".to_owned(),
        }
    }

    pub fn run(&self) -> Ran {
        let mut child = Command::new(&self.outer)
            .arg("-c")
            .arg(&self.command)
            .env_clear()
            .env("HOME", &self.home)
            .env("TMPDIR", &self.tmp)
            .env("PATH", &self.path)
            .env("SHELL", &self.shell)
            .env("TERM", "xterm-256color")
            .env("LANG", "C")
            // bash writes its history on exit; keep that out of the scratch HOME's reach.
            .env("HISTFILE", "/dev/null")
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn bootstrap");
        if let Some(mut stdin) = child.stdin.take() {
            // The shell may exit before reading everything, so a broken pipe is not an error.
            let _ = stdin.write_all(self.input.as_bytes());
        }
        let output = child.wait_with_output().expect("wait");
        let mut text = output.stdout;
        text.extend(output.stderr);
        Ran {
            status: output.status,
            text,
        }
    }
}

/// Runs the bootstrap as `sh -c <remote>` with a scratch HOME, TMPDIR, PATH and SHELL, and
/// `input` on its stdin (what the person types).
pub fn run(s: &Scratch, path: &str, shell: &str, input: &str) -> Ran {
    let mut launch = Launch::new(s);
    launch.path = path.to_owned();
    launch.shell = shell.to_owned();
    launch.input = input.to_owned();
    launch.run()
}

/// The machine's host name as the bootstrap's `hostname` reports it.
pub fn host_name() -> String {
    let out = Command::new("hostname").output().expect("hostname");
    String::from_utf8_lossy(&out.stdout).trim_end().to_owned()
}

/// The system PATH, which has `bash`, `sh` and the coreutils.
pub fn system_path() -> String {
    std::env::var("PATH").unwrap_or_default()
}

pub fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// [`StdRunner`] that tries again when the program cannot be started because it is still open
/// for writing: a fake tool written just before, while another test's thread forked and let
/// the child inherit the write handle (ETXTBSY). The handle closes as that child execs.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetryRunner;

impl CommandRunner for RetryRunner {
    fn run(&self, program: &Path, args: &[String]) -> io::Result<CommandOutput> {
        let mut tries = 0;
        loop {
            match StdRunner.run(program, args) {
                Err(err) if err.kind() == io::ErrorKind::ExecutableFileBusy && tries < 50 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                done => return done,
            }
        }
    }
}

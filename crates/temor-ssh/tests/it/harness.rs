//! Scratch directories, fake tools on a private PATH, and running the bootstrap.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use temor_pty::integration_scripts;
use temor_ssh::{BootstrapScript, RemoteCommand, bootstrap_remote_command};

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
    bootstrap_remote_command(&script)
}

pub struct Ran {
    pub status: ExitStatus,
    /// stdout and stderr together, since the shell prompts on stderr.
    pub text: Vec<u8>,
}

/// Runs the bootstrap as `sh -c <remote>` with a scratch HOME, TMPDIR, PATH and SHELL, and
/// `input` on its stdin (what the person types).
pub fn run(s: &Scratch, path: &str, shell: &str, input: &str) -> Ran {
    let sh = tool("sh").expect("sh on the test PATH");
    let mut child = Command::new(sh)
        .arg("-c")
        .arg(remote().as_str())
        .env_clear()
        .env("HOME", &s.home)
        .env("TMPDIR", &s.tmp)
        .env("PATH", path)
        .env("SHELL", shell)
        .env("TERM", "xterm-256color")
        .env("LANG", "C")
        // bash writes its history on exit; keep that inside the scratch HOME's reach, not the
        // dotfile snapshot.
        .env("HISTFILE", "/dev/null")
        .current_dir(&s.work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bootstrap");
    if let Some(mut stdin) = child.stdin.take() {
        // The shell may exit before reading everything, so a broken pipe is not an error.
        let _ = stdin.write_all(input.as_bytes());
    }
    let output = child.wait_with_output().expect("wait");
    let mut text = output.stdout;
    text.extend(output.stderr);
    Ran {
        status: output.status,
        text,
    }
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

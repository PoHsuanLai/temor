//! Git as a child process, from a binary path the caller injects (found on `PATH` by the app),
//! behind temor-core's `Git` trait: the core stays free of I/O (PLAN §2, L2).

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use temor_core::error::CoreError;
use temor_core::worktree::{Git, GitCommand, GitOutput};

/// Runs the git at `binary`. Terminal prompts are off and the language is `C`, so output is
/// readable and a missing credential fails instead of waiting.
#[derive(Debug, Clone)]
pub struct ProcessGit {
    binary: PathBuf,
    env: Vec<(String, String)>,
}

impl ProcessGit {
    /// A runner for the git at `binary`.
    pub fn new(binary: impl Into<PathBuf>) -> ProcessGit {
        ProcessGit {
            binary: binary.into(),
            env: Vec::new(),
        }
    }

    /// This runner with one more environment variable on every run (tests set the identity and
    /// `GIT_CONFIG_GLOBAL`).
    #[must_use]
    pub fn with_env(mut self, key: &str, value: &str) -> ProcessGit {
        self.env.push((key.to_owned(), value.to_owned()));
        self
    }
}

impl ProcessGit {
    fn command(&self, cmd: &GitCommand) -> Command {
        let mut process = Command::new(&self.binary);
        process
            .arg("-C")
            .arg(&cmd.dir)
            .args(&cmd.args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .stdin(Stdio::null());
        for (key, value) in self.env.iter().chain(cmd.env.iter()) {
            process.env(key, value);
        }
        process
    }
}

impl Git for ProcessGit {
    fn run(&self, cmd: &GitCommand) -> Result<GitOutput, CoreError> {
        let out = self
            .command(cmd)
            .output()
            .map_err(|e| CoreError::GitNotRun(e.to_string()))?;
        Ok(GitOutput {
            code: out.status.code().unwrap_or(-1),
            stdout: out.stdout,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn run_capped(
        &self,
        cmd: &GitCommand,
        prefix: &[u8],
        count: usize,
    ) -> Result<GitOutput, CoreError> {
        let mut child = self
            .command(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CoreError::GitNotRun(e.to_string()))?;
        let mut stdout = Vec::new();
        let mut cut = false;
        if let Some(pipe) = child.stdout.take() {
            let mut reader = BufReader::new(pipe);
            let mut seen = 0;
            while seen < count {
                let from = stdout.len();
                match reader.read_until(b'\n', &mut stdout) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => seen += usize::from(stdout[from..].starts_with(prefix)),
                }
            }
            // Past the cap git still has work to do on a huge folder: stop it now.
            cut = seen >= count;
        }
        if cut {
            let _ = child.kill();
        }
        let status = child
            .wait()
            .map_err(|e| CoreError::GitNotRun(e.to_string()))?;
        let code = match cut {
            true => 0,
            false => status.code().unwrap_or(-1),
        };
        Ok(GitOutput {
            code,
            stdout,
            stderr: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn a_capped_run_stops_git_that_never_stops_printing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let git = dir.path().join("git");
        std::fs::write(&git, "#!/bin/sh\necho ' M kept'\nexec yes '?? f'\n").expect("script");
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let start = Instant::now();
        let cmd = GitCommand::new(&dir.path().to_string_lossy(), &["status"]);
        let out = ProcessGit::new(&git)
            .run_capped(&cmd, b"?? ", 1000)
            .expect("ran");
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "git was not stopped"
        );
        assert_eq!(out.code, 0);
        let text = out.text();
        assert_eq!(
            text.lines().count(),
            1001,
            "the tracked line and 1000 new ones"
        );
        assert!(text.starts_with(" M kept\n"));
    }
}

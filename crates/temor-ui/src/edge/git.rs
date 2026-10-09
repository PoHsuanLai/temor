//! Git as a child process, from a binary path the caller injects (found on `PATH` by the app),
//! behind temor-core's `Git` trait: the core stays free of I/O (PLAN §2, L2).

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

impl Git for ProcessGit {
    fn run(&self, cmd: &GitCommand) -> Result<GitOutput, CoreError> {
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
        let out = process
            .output()
            .map_err(|e| CoreError::GitNotRun(e.to_string()))?;
        Ok(GitOutput {
            code: out.status.code().unwrap_or(-1),
            stdout: out.stdout,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

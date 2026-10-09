//! The one seam to git: a command as data, and a trait that runs it. The core builds
//! [`GitCommand`]s and reads [`GitOutput`]s; what actually starts a process is a [`Git`]
//! implementation chosen at the edge (`ProcessGit`, or a fake in tests).

use crate::error::CoreError;

/// One git invocation: where, with which arguments and extra environment variables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitCommand {
    /// The directory to run in (`git -C <dir>`).
    pub dir: String,
    /// The arguments after `git`.
    pub args: Vec<String>,
    /// Environment variables for this run, such as `GIT_INDEX_FILE`.
    pub env: Vec<(String, String)>,
}

impl GitCommand {
    /// `git <args>` in `dir`.
    pub fn new(dir: &str, args: &[&str]) -> GitCommand {
        GitCommand {
            dir: dir.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: Vec::new(),
        }
    }

    /// This command with one more environment variable.
    #[must_use]
    pub fn with_env(mut self, key: &str, value: &str) -> GitCommand {
        self.env.push((key.to_owned(), value.to_owned()));
        self
    }

    /// The first two arguments, for messages.
    pub fn what(&self) -> String {
        self.args
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What a git run ended with. A non-zero code is data here, because `merge-tree` and `diff`
/// use it to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    /// The exit code; `-1` when a signal ended it.
    pub code: i32,
    /// Standard output, bytes (paths may not be UTF-8).
    pub stdout: Vec<u8>,
    /// Standard error, lossy.
    pub stderr: String,
}

impl GitOutput {
    /// Standard output as text, lossy.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// This output when the code is one of `ok`, else the failure to report for `cmd`.
    pub fn expect_code(self, cmd: &GitCommand, ok: &[i32]) -> Result<GitOutput, CoreError> {
        match ok.contains(&self.code) {
            true => Ok(self),
            false => Err(CoreError::GitFailed {
                what: cmd.what(),
                stderr: self.stderr.chars().take(300).collect(),
            }),
        }
    }
}

/// Runs git. `Send + Sync` so the runner can sit on a worker thread the caller owns.
pub trait Git: Send + Sync {
    /// Runs `cmd` to completion. A non-zero exit is `Ok`; only failing to start git is `Err`.
    fn run(&self, cmd: &GitCommand) -> Result<GitOutput, CoreError>;
}

/// Runs `cmd` and requires exit code 0.
pub fn run_ok(git: &dyn Git, cmd: &GitCommand) -> Result<GitOutput, CoreError> {
    git.run(cmd)?.expect_code(cmd, &[0])
}

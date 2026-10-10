//! The edge: running the commands. [`RemoteFiles`] is what the app (and the edit machine in
//! temor-core) needs; [`SshRemoteFiles`] implements it with `ssh` and `scp` through a
//! [`CommandRunner`], which tests replace with a fake.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use super::argv::{FileTools, fetch_argv, stat_argv, store_argv};
use super::path::{RemoteFilePath, RemoteStat, RemoteState, parse_stat};
use crate::argv::ControlLayout;
use crate::config::HostAlias;
use crate::error::SshError;

/// What a finished program left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// The exit code; `None` when a signal ended it.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs a program to completion with no input. The app uses [`StdRunner`] on a worker thread;
/// a call blocks until the program ends.
pub trait CommandRunner {
    fn run(&self, program: &Path, args: &[String]) -> io::Result<CommandOutput>;
}

/// Runs the program as a child process of this one.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdRunner;

impl CommandRunner for StdRunner {
    fn run(&self, program: &Path, args: &[String]) -> io::Result<CommandOutput> {
        let out = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        Ok(CommandOutput {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }
}

/// The three operations a remote edit needs, all on the shared connection to one host. They
/// block; call them off the UI thread. Each fails (rather than asking for a password) when the
/// master is not running.
pub trait RemoteFiles {
    /// The file's modified time and size, or that it is gone.
    fn stat(&self, host: &HostAlias, path: &RemoteFilePath) -> Result<RemoteState, SshError>;
    /// Copies the host's file to the absolute `local` path.
    fn fetch(&self, host: &HostAlias, path: &RemoteFilePath, local: &Path) -> Result<(), SshError>;
    /// Copies the absolute `local` file over the host's file.
    fn store(&self, host: &HostAlias, local: &Path, path: &RemoteFilePath) -> Result<(), SshError>;

    /// Opens a file for editing: its stat first, then its content, so a change made between the
    /// two shows up as a conflict on the first save instead of being lost. `None` when the file
    /// does not exist.
    fn download(
        &self,
        host: &HostAlias,
        path: &RemoteFilePath,
        local: &Path,
    ) -> Result<Option<RemoteStat>, SshError> {
        match self.stat(host, path)? {
            RemoteState::Present(stat) => {
                self.fetch(host, path, local)?;
                Ok(Some(stat))
            }
            RemoteState::Missing => Ok(None),
        }
    }
}

/// [`RemoteFiles`] with the system `ssh` and `scp`.
#[derive(Debug, Clone)]
pub struct SshRemoteFiles<R> {
    runner: R,
    tools: FileTools,
    layout: ControlLayout,
}

impl<R: CommandRunner> SshRemoteFiles<R> {
    /// Files over the master that `layout` places.
    pub fn new(runner: R, tools: FileTools, layout: ControlLayout) -> Self {
        Self {
            runner,
            tools,
            layout,
        }
    }

    fn run(
        &self,
        program: &Path,
        what: &'static str,
        args: &[String],
    ) -> Result<CommandOutput, SshError> {
        let out = self
            .runner
            .run(program, args)
            .map_err(|source| SshError::Spawn {
                program: program.display().to_string(),
                source,
            })?;
        match out.code {
            Some(0) => Ok(out),
            code => Err(SshError::RemoteFile {
                what,
                code,
                detail: first_line(&out.stderr),
            }),
        }
    }
}

impl<R: CommandRunner> RemoteFiles for SshRemoteFiles<R> {
    fn stat(&self, host: &HostAlias, path: &RemoteFilePath) -> Result<RemoteState, SshError> {
        let argv = stat_argv(host, &self.layout, path)?;
        let out = self.run(&self.tools.ssh, "reading the file's details", &argv)?;
        parse_stat(&String::from_utf8_lossy(&out.stdout))
    }

    fn fetch(&self, host: &HostAlias, path: &RemoteFilePath, local: &Path) -> Result<(), SshError> {
        let argv = fetch_argv(host, &self.layout, path, local)?;
        self.run(&self.tools.scp, "copying the file from the computer", &argv)
            .map(drop)
    }

    fn store(&self, host: &HostAlias, local: &Path, path: &RemoteFilePath) -> Result<(), SshError> {
        let argv = store_argv(host, &self.layout, local, path)?;
        self.run(&self.tools.scp, "copying the file to the computer", &argv)
            .map(drop)
    }
}

/// The first non-empty line of `stderr`, without control characters, cut at 200 characters.
fn first_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect()
}

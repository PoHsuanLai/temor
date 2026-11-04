//! Running a program for real: the one [`CommandRunner`] that starts a process.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use super::edge::{CommandOutput, CommandRunner};

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

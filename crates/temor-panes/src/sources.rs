//! The sources that complete the composer's line besides the running shell's own (which the
//! session drives through the pane, see `composer`): each stands on its own, so it can run
//! anywhere a line and a folder can be taken, and answers from a thread of its own.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use temor_shell::ShellKind;
use temor_shell::suggest::{
    Answered, Ask, CompletionSource, Deliver, SourceId, Suggestion, read_nushell,
};

/// The running shell's own completion, through the pane.
pub const SHELL: SourceId = SourceId("shell");

/// Nushell's.
const NUSHELL: SourceId = SourceId("nushell");

/// How long nushell has to answer before it is given up on.
const NUSHELL_PATIENCE: Duration = Duration::from_secs(3);

/// The sources a session has, and how they hand an answer back.
#[derive(Default)]
pub struct Sources {
    pub list: Vec<Box<dyn CompletionSource>>,
    pub deliver: Option<Deliver>,
}

impl std::fmt::Debug for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sources")
            .field("list", &self.list.len())
            .finish_non_exhaustive()
    }
}

impl Sources {
    /// The sources for a local pane running `program`: nushell has no completion that can be
    /// asked from inside it, so it is asked from outside.
    pub fn for_program(program: &Path, deliver: Deliver) -> Self {
        let name = program.file_name().and_then(|name| name.to_str());
        let kind = ShellKind::from_process_name(name.unwrap_or_default());
        let mut list: Vec<Box<dyn CompletionSource>> = Vec::new();
        if kind == ShellKind::Nushell {
            list.push(Box::new(NushellSource {
                program: program.to_owned(),
                patience: NUSHELL_PATIENCE,
            }));
        }
        Self {
            list,
            deliver: Some(deliver),
        }
    }

    /// Whether any source can answer now.
    pub fn any_ready(&self) -> bool {
        self.deliver.is_some() && self.list.iter().any(|source| source.ready())
    }
}

/// Nushell's own completion, which `nu --ide-complete` prints for a script and a place in it: the
/// line is written to a file and nushell is run (without the person's config, which would make it
/// slow to start) in the pane's folder.
#[derive(Debug)]
pub struct NushellSource {
    program: PathBuf,
    patience: Duration,
}

impl CompletionSource for NushellSource {
    fn id(&self) -> SourceId {
        NUSHELL
    }

    fn ask(&self, ask: Ask, request: u64, deliver: Deliver) {
        let (program, patience) = (self.program.clone(), self.patience);
        let fallback = deliver.clone();
        let spawned = std::thread::Builder::new()
            .name("temor-complete-nu".to_owned())
            .spawn(move || {
                let suggestions = run_nushell(&program, &ask, patience).unwrap_or_default();
                deliver(Answered {
                    request,
                    source: NUSHELL,
                    suggestions,
                });
            });
        if spawned.is_err() {
            fallback(Answered {
                request,
                source: NUSHELL,
                suggestions: Vec::new(),
            });
        }
    }
}

/// What nushell completes `ask.line` to, or `None` when it did not say in time.
fn run_nushell(program: &Path, ask: &Ask, patience: Duration) -> Option<Vec<Suggestion>> {
    let script = tempfile::Builder::new().suffix(".nu").tempfile().ok()?;
    std::fs::write(script.path(), &ask.line).ok()?;
    let mut command = Command::new(program);
    command
        .args(["--no-config-file", "--ide-complete"])
        .arg(ask.line.len().to_string())
        .arg(script.path())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    if let Some(folder) = ask.folder.as_deref().filter(|folder| folder.is_dir()) {
        command.current_dir(folder);
    }
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    // Read apart from the wait, so a long answer cannot fill the pipe and stall the program.
    let (read, output) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = String::new();
        let _ = read.send(stdout.read_to_string(&mut out).ok().map(|_| out));
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < patience => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    // A program that was killed may have left one running that holds the pipe: not waited for.
    let output = output.recv_timeout(Duration::from_millis(500)).ok()??;
    read_nushell(&output, &ask.line).ok()
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;

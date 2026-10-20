//! A program run in a tab of its own in place of the shell (`temor -e`, the terminal-exec
//! convention): the launch that asked gives its words, its folder and its environment, and the tab
//! keeps them so Restart runs the same program again. The tab stays when the program ends, until
//! the person closes it; then the launch, which waits on its ticket, hears the status to exit
//! with: the program's last, or a hang-up's when the tab closed while it ran.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use temor_core::vocab::ids::PaneId;
use temor_desktop::{CLOSED_WHILE_RUNNING, NOT_FOUND, RunEnds, RunRequest, RunTicket};
use temor_pty::ChildExit;
use temor_shell::ChildEnv;

use super::desktop::cwd_of;
use super::model::Hub;
use crate::view::launch::Program;

/// A program to run and where, as the launch that asked for it had them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The program and its arguments; never empty.
    pub argv: Vec<OsString>,
    /// The folder to run it in; the default folder when `None`.
    pub cwd: Option<PathBuf>,
    /// The environment it runs with, over temor's own.
    pub env: ChildEnv,
    /// The launch waiting for the tab to close, if one waits.
    pub ticket: Option<RunTicket>,
}

impl From<RunRequest> for Run {
    fn from(request: RunRequest) -> Run {
        Run {
            argv: request.argv,
            cwd: request.cwd,
            env: request.env.into_iter().collect(),
            ticket: request.ticket,
        }
    }
}

/// A run whose program was found: what the pane starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: ChildEnv,
    /// The name the person knows it by, for "… has ended".
    pub name: String,
    pub ticket: Option<RunTicket>,
    /// How the program last ended; `None` while it runs.
    pub status: Option<i32>,
}

impl Found {
    pub(super) fn program(&self) -> Program {
        Program {
            program: self.program.clone(),
            args: self.args.clone(),
            tap: None,
        }
    }
}

/// The run waiting for the next pane, and the panes that run a program.
#[derive(Debug, Default)]
pub(super) struct RunHub {
    /// Taken by the next pane that starts.
    pub intent: Option<Found>,
    pub panes: BTreeMap<PaneId, Found>,
    /// Where a closed run tab is told to its waiting launch.
    pub ends: RunEnds,
}

impl Hub {
    /// Opens a tab that runs `run` in place of the shell. A program that cannot be found is
    /// said, and no tab opens.
    pub fn run_in_tab(&mut self, run: Run) {
        let Some(found) = self.find_run(&run) else {
            // As a shell says it: the launch exits at once, with no tab to wait for.
            if let Some(ticket) = run.ticket {
                self.run.ends.ended(ticket, NOT_FOUND);
            }
            return;
        };
        self.run.intent = Some(found);
        let _ = self.open_tab(run.cwd.as_deref().map(cwd_of));
        // A workspace that refused the tab leaves nothing waiting for an unrelated pane.
        self.run.intent = None;
    }

    /// The program `run` names, looked for as a shell would: a name with a slash is a path (from
    /// the run's folder), a bare name is looked for in the run's `PATH`, else temor's.
    fn find_run(&mut self, run: &Run) -> Option<Found> {
        let (first, args) = run.argv.split_first()?;
        let name = first.to_string_lossy().into_owned();
        let path = match name.contains('/') {
            true => Some(within(run.cwd.as_deref(), Path::new(first))),
            false => {
                let search = run
                    .env
                    .get("PATH")
                    .or_else(|| self.launch.ambient.env.get("PATH"))
                    .unwrap_or_default()
                    .to_owned();
                temor_pty::find_program(&name, &search)
            }
        };
        let Some(program) = path.filter(|path| path.is_file()) else {
            self.notices
                .push(format!("temor couldn't find the program “{name}”."));
            return None;
        };
        Some(Found {
            program,
            args: args.to_vec(),
            env: run.env.clone(),
            name: shown_name(&name),
            ticket: run.ticket,
            status: None,
        })
    }

    /// The run a pane about to start takes, if one is waiting; the pane keeps it for Restart.
    pub(super) fn run_claims(&mut self, pane: PaneId) -> Option<Found> {
        let found = self.run.intent.take()?;
        self.run.panes.insert(pane, found.clone());
        Some(found)
    }

    /// Where a closed run tab is told to the launch that waits on it.
    pub fn set_run_ends(&mut self, ends: RunEnds) {
        self.run.ends = ends;
    }

    /// `pane`'s program ended `exit`; a run tab keeps it for its launch.
    pub(super) fn run_ended(&mut self, pane: PaneId, exit: Option<ChildExit>) {
        if let Some(found) = self.run.panes.get_mut(&pane) {
            found.status = Some(status_of(exit));
        }
    }

    /// `pane`'s program is starting again (Restart): it runs, so nothing has ended yet.
    pub(super) fn run_restarted(&mut self, pane: PaneId) {
        if let Some(found) = self.run.panes.get_mut(&pane) {
            found.status = None;
        }
    }

    /// `pane` is gone: a run tab's launch stops waiting.
    pub(super) fn run_closed(&mut self, pane: PaneId) {
        let Some(found) = self.run.panes.remove(&pane) else {
            return;
        };
        if let Some(ticket) = found.ticket {
            self.run
                .ends
                .ended(ticket, found.status.unwrap_or(CLOSED_WHILE_RUNNING));
        }
    }

    /// The name of the program `pane` runs in place of the shell, if it does.
    pub fn run_name(&self, pane: PaneId) -> Option<&str> {
        self.run.panes.get(&pane).map(|found| found.name.as_str())
    }
}

/// `path` from `dir` when it is relative.
fn within(dir: Option<&Path>, path: &Path) -> PathBuf {
    match (dir, path.is_relative()) {
        (Some(dir), true) => dir.join(path),
        _ => path.to_path_buf(),
    }
}

/// The last part of what was typed: `htop` for `/usr/bin/htop`.
fn shown_name(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_owned()
}

/// The status a shell would report for `exit`: the code, 128 plus a signal, or a hang-up's when
/// it could not be read.
fn status_of(exit: Option<ChildExit>) -> i32 {
    match exit {
        Some(ChildExit::Exited(code)) => code,
        Some(ChildExit::Signaled(signal)) => 128 + signal,
        None => CLOSED_WHILE_RUNNING,
    }
}

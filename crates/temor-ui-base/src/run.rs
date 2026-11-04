//! A program run in a tab of its own in place of the shell (`temor -e`), as the launch that asked
//! for it gave it. The hub keeps it with the tab (`temor_hub::run`).

use std::ffi::OsString;
use std::path::PathBuf;

use temor_client::desktop::{RunRequest, RunTicket};
use temor_panes::shell::ChildEnv;

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

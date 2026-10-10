//! The argv vectors. Every value is its own argument and no shell sees any of them. The
//! destination follows `--`, so a name can never be read as an option.

use super::control::ControlLayout;
use super::forward::Forward;
use crate::config::HostAlias;

/// A command for the remote login shell, as one string. Built only by the bootstrap, which
/// quotes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCommand(String);

impl RemoteCommand {
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    /// The command text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The ControlMaster options. `ControlMaster=auto` makes this session the master when none is
/// running, and a later session joins it.
fn shared(layout: &ControlLayout) -> Vec<String> {
    vec![
        "-o".into(),
        "ControlMaster=auto".into(),
        "-o".into(),
        layout.control_path_option(),
        "-o".into(),
        layout.control_persist_option(),
    ]
}

/// The control-socket options alone, for `-O` commands that talk to a running master.
fn control_only(layout: &ControlLayout) -> Vec<String> {
    vec!["-o".into(), layout.control_path_option()]
}

/// A session on the shared connection to `alias`, with a pty (`-t`) running `remote`. The
/// first session opens the master; later ones join it at once.
pub fn session_argv(
    alias: &HostAlias,
    layout: &ControlLayout,
    remote: &RemoteCommand,
) -> Vec<String> {
    let mut argv = vec!["-t".to_owned()];
    argv.extend(shared(layout));
    argv.extend([
        "--".to_owned(),
        alias.as_str().to_owned(),
        remote.as_str().to_owned(),
    ]);
    argv
}

/// A plain session on the shared connection to `alias`: a pty (`-t`) and no remote command, so
/// the host runs the person's own login shell and nothing of temor's. Used when the person
/// declined the setup.
pub fn plain_session_argv(alias: &HostAlias, layout: &ControlLayout) -> Vec<String> {
    let mut argv = vec!["-t".to_owned()];
    argv.extend(shared(layout));
    argv.extend(["--".to_owned(), alias.as_str().to_owned()]);
    argv
}

/// `ssh -O check`: is the master for `alias` running?
pub fn check_argv(alias: &HostAlias, layout: &ControlLayout) -> Vec<String> {
    control_command("check", alias, layout, Vec::new())
}

/// `ssh -O exit`: ask the master for `alias` to close.
pub fn exit_argv(alias: &HostAlias, layout: &ControlLayout) -> Vec<String> {
    control_command("exit", alias, layout, Vec::new())
}

/// `ssh -O forward -L …`: open a forward on the running master.
pub fn forward_argv(alias: &HostAlias, layout: &ControlLayout, forward: &Forward) -> Vec<String> {
    control_command(
        "forward",
        alias,
        layout,
        vec!["-L".to_owned(), forward.spec().as_str().to_owned()],
    )
}

/// `ssh -O cancel -L …`: close a forward opened with [`forward_argv`].
pub fn cancel_forward_argv(
    alias: &HostAlias,
    layout: &ControlLayout,
    forward: &Forward,
) -> Vec<String> {
    control_command(
        "cancel",
        alias,
        layout,
        vec!["-L".to_owned(), forward.spec().as_str().to_owned()],
    )
}

fn control_command(
    op: &str,
    alias: &HostAlias,
    layout: &ControlLayout,
    extra: Vec<String>,
) -> Vec<String> {
    let mut argv = vec!["-O".to_owned(), op.to_owned()];
    argv.extend(control_only(layout));
    argv.extend(extra);
    argv.extend(["--".to_owned(), alias.as_str().to_owned()]);
    argv
}

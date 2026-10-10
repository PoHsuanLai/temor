//! The SSH jobs that wait on a program or the network. Each runs on a thread of its own and
//! reports back as an [`SshEvent`], which the hub reads on the window's thread (the same way a
//! pane's events arrive). The jobs only run the programs the pure steps in `temor-core` and the
//! argv builders in `temor-ssh` name.

use std::net::TcpListener;
use std::path::PathBuf;

use temor_core::host::edit::{EditFault, EditIn};
use temor_core::host::forward::ForwardFault;
use temor_ssh::{
    CommandRunner, ControlLayout, Fingerprint, Forward, HostAlias, HostName, MasterProbe, Port,
    RemoteFilePath, RemoteFiles, RemoteState, SshError, SshRemoteFiles, cancel_forward_argv,
    fingerprint_from_keygen, forward_argv, keygen_lookup_argv, probe_master,
};
use temor_wire::message::FileStamp;
use tokio::sync::mpsc::UnboundedSender;

use super::SshEnv;

/// One remote file being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EditId(pub u64);

/// What a job found.
#[derive(Debug)]
pub enum SshEvent {
    /// Whether a host's shared connection is running.
    Probed { host: HostAlias, probe: MasterProbe },
    /// The fingerprint of the key a host's connection verified, when it could be read.
    Identified {
        host: HostAlias,
        fingerprint: Option<Fingerprint>,
    },
    /// A step of a remote edit finished.
    Edit { id: EditId, input: EditIn },
    /// The host's file, as a helper read or wrote it: the stamp the next write-back must still
    /// find. Sent just before the `Edit` event that goes with it.
    EditSeen { id: EditId, stamp: FileStamp },
    /// The forward of the host's `remote` port is up.
    ForwardOpened { host: HostAlias, remote: Port },
    /// The forward of the host's `remote` port could not be opened.
    ForwardFailed {
        host: HostAlias,
        remote: Port,
        fault: ForwardFault,
    },
}

/// Starts jobs. Cloning shares the channel.
#[derive(Debug, Clone)]
pub struct SshWork {
    env: SshEnv,
    events: UnboundedSender<SshEvent>,
}

impl SshWork {
    pub fn new(env: SshEnv, events: UnboundedSender<SshEvent>) -> Self {
        Self { env, events }
    }

    /// The channel the jobs report on, for jobs that run through the host's helper.
    pub fn sender(&self) -> UnboundedSender<SshEvent> {
        self.events.clone()
    }

    /// Runs `job` on a thread. If the thread cannot start, `failed` is sent instead.
    fn run(
        &self,
        job: impl FnOnce(&SshEnv, &ControlLayout) -> Option<SshEvent> + Send + 'static,
        failed: Option<SshEvent>,
    ) {
        let (env, events) = (self.env.clone(), self.events.clone());
        let started = std::thread::Builder::new()
            .name("temor-ssh-job".to_owned())
            .spawn(move || {
                let reply = env
                    .layout
                    .as_ref()
                    .ok()
                    .and_then(|layout| job(&env, layout));
                if let Some(reply) = reply {
                    let _ = events.send(reply);
                }
            });
        if let (Err(_), Some(failed)) = (started, failed) {
            let _ = self.events.send(failed);
        }
    }

    /// Asks whether `host`'s shared connection is running.
    pub fn probe(&self, host: HostAlias) {
        let gone = SshEvent::Probed {
            host: host.clone(),
            probe: MasterProbe::NotRunning,
        };
        self.run(
            move |env, layout| {
                let probe = probe_master(&env.runner, &env.tools, &host, layout)
                    .unwrap_or(MasterProbe::NotRunning);
                Some(SshEvent::Probed { host, probe })
            },
            Some(gone),
        );
    }

    /// Reads the fingerprint of the key recorded for `name` (the host name the connection went
    /// to). Ask after the connection verified the key.
    pub fn identify(&self, host: HostAlias, name: HostName, port: Option<Port>) {
        let none = SshEvent::Identified {
            host: host.clone(),
            fingerprint: None,
        };
        self.run(
            move |env, _| {
                let argv = keygen_lookup_argv(&name, port);
                let fingerprint = env.runner.run(&env.keygen, &argv).ok().and_then(|out| {
                    fingerprint_from_keygen(&String::from_utf8_lossy(&out.stdout)).ok()
                });
                Some(SshEvent::Identified { host, fingerprint })
            },
            Some(none),
        );
    }

    /// Copies the host's file to `local` (stat first), then reports `Fetched`, `FetchGone` or a
    /// fault.
    pub fn fetch(&self, id: EditId, host: HostAlias, path: RemoteFilePath, local: PathBuf) {
        let input = move |input| SshEvent::Edit { id, input };
        self.run(
            move |env, layout| {
                let files = files_of(env, layout);
                Some(input(match files.download(&host, &path, &local) {
                    Ok(Some(stat)) => EditIn::Fetched { stat },
                    Ok(None) => EditIn::FetchGone,
                    Err(err) => EditIn::Failed(edit_fault(&err)),
                }))
            },
            Some(input(EditIn::Failed(EditFault::NoConnection))),
        );
    }

    /// Reads the host file's modified time and size.
    pub fn check(&self, id: EditId, host: HostAlias, path: RemoteFilePath) {
        let input = move |input| SshEvent::Edit { id, input };
        self.run(
            move |env, layout| {
                Some(input(match files_of(env, layout).stat(&host, &path) {
                    Ok(state) => EditIn::Checked(state),
                    Err(err) => EditIn::Failed(edit_fault(&err)),
                }))
            },
            Some(input(EditIn::Failed(EditFault::NoConnection))),
        );
    }

    /// Copies `local` over the host's file, then reads the file's new stat.
    pub fn store(&self, id: EditId, host: HostAlias, local: PathBuf, path: RemoteFilePath) {
        let input = move |input| SshEvent::Edit { id, input };
        self.run(
            move |env, layout| {
                let files = files_of(env, layout);
                let done = files
                    .store(&host, &local, &path)
                    .and_then(|()| files.stat(&host, &path));
                Some(input(match done {
                    Ok(RemoteState::Present(stat)) => EditIn::Stored { stat },
                    Ok(_) => EditIn::Failed(EditFault::Gone),
                    Err(err) => EditIn::Failed(edit_fault(&err)),
                }))
            },
            Some(input(EditIn::Failed(EditFault::NoConnection))),
        );
    }

    /// Opens `forward` on `host`'s shared connection.
    pub fn open_forward(&self, host: HostAlias, forward: Forward) {
        let remote = forward.remote;
        let failed = SshEvent::ForwardFailed {
            host: host.clone(),
            remote,
            fault: ForwardFault::NoConnection,
        };
        self.run(
            move |env, layout| {
                let argv = forward_argv(&host, layout, &forward);
                Some(match env.runner.run(&env.tools.ssh, &argv) {
                    Ok(out) if out.code == Some(0) => SshEvent::ForwardOpened { host, remote },
                    Ok(out) => SshEvent::ForwardFailed {
                        host,
                        remote,
                        fault: forward_fault(&String::from_utf8_lossy(&out.stderr)),
                    },
                    Err(_) => SshEvent::ForwardFailed {
                        host,
                        remote,
                        fault: ForwardFault::NoConnection,
                    },
                })
            },
            Some(failed),
        );
    }

    /// Closes `forward` on `host`'s shared connection. Its result does not matter.
    pub fn close_forward(&self, host: HostAlias, forward: Forward) {
        self.run(
            move |env, layout| {
                let argv = cancel_forward_argv(&host, layout, &forward);
                let _ = env.runner.run(&env.tools.ssh, &argv);
                None
            },
            None,
        );
    }
}

fn files_of(env: &SshEnv, layout: &ControlLayout) -> SshRemoteFiles<super::ArcRunner> {
    SshRemoteFiles::new(env.runner.clone(), env.tools.clone(), layout.clone())
}

/// How a failed file call reads to the person: no connection (ssh itself failed, exit 255, or
/// could not start) or a refusal by the host.
pub fn edit_fault(err: &SshError) -> EditFault {
    match err {
        SshError::RemoteFile {
            code: Some(255), ..
        }
        | SshError::Spawn { .. } => EditFault::NoConnection,
        _ => EditFault::Refused,
    }
}

/// Why `ssh -O forward` failed, from what it printed.
pub fn forward_fault(stderr: &str) -> ForwardFault {
    let said = stderr.to_ascii_lowercase();
    let has = |needle: &str| said.contains(needle);
    if has("address already in use") || has("cannot listen") {
        ForwardFault::LocalPortTaken
    } else if has("control socket") || has("mux_client") || has("no such file") {
        ForwardFault::NoConnection
    } else {
        ForwardFault::Refused
    }
}

/// A port on this computer to forward to: `preferred` when it is free, else any free one.
pub fn free_local_port(preferred: Port) -> Option<Port> {
    let listen = |port: u16| TcpListener::bind(("127.0.0.1", port)).ok();
    let chosen = listen(preferred.get())
        .and_then(|l| l.local_addr().ok())
        .or_else(|| listen(0).and_then(|l| l.local_addr().ok()))?;
    Port::parse(&chosen.port().to_string())
}

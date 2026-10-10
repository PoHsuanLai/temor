//! The jobs behind a Tier B session that wait on `ssh` or the network (PLAN §9b): finding out what
//! a host has, putting the helper there, connecting to it, and opening or reattaching a pane's
//! session. Each runs on a thread of its own and reports as a [`RemoteEvent`], which the hub
//! reads on the window's thread the way it reads a pane's events. The decisions are the pure
//! steps in temor-core and temor-wire; these only run what they name.

use std::process::{Command, Stdio};
use std::sync::Arc;

use temor_core::remote::Probe;
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::HelperHash;
use temor_pane::Feeder;
use temor_pty::ChildExit;
use temor_remote::Error;
use temor_ssh::{CommandOutput, CommandRunner, HostAlias};
use temor_wire::digest::ContentHash;
use temor_wire::ids::SessionId;
use temor_wire::message::{ListeningPort, RemoteError};
use tokio::sync::mpsc::UnboundedSender;

use super::SshEnv;
use super::remote_calls::{attach, close, open, ports, resize};
use super::remote_conn::{Conn, DialFault, dial, dial_argv};
use super::remote_install::{
    HelperBlob, PROBE_SCRIPT, command_argv, install_script, parse_probe, remove_script,
};
use super::remote_link::{SharedTarget, lock};
use super::remote_pump::pump;
use super::remote_route::Router;

/// Which connection of a host an event is about; an event of an older one is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Generation(pub u64);

impl Generation {
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// Why a pane's session could not be opened or reattached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachFault {
    /// The host no longer has the session (it ended, or its helper was restarted).
    Gone,
    /// The call failed; the link is probably down.
    Failed,
}

/// What a job found, or what a connection told.
#[derive(Debug)]
pub enum RemoteEvent {
    /// What the host has; `None` when it could not be asked.
    Probed {
        host: HostAlias,
        probe: Option<Probe>,
    },
    /// The helper was stored; the hash of what is there now, `None` when the upload failed.
    Installed {
        host: HostAlias,
        hash: Option<HelperHash>,
    },
    /// A dial finished.
    Dialed {
        host: HostAlias,
        generation: Generation,
        outcome: Result<Arc<Conn>, DialFault>,
    },
    /// A pane's session is open and attached, its screen handed to the pane.
    Attached {
        host: HostAlias,
        generation: Generation,
        pane: PaneId,
        session: SessionId,
    },
    AttachFailed {
        host: HostAlias,
        generation: Generation,
        pane: PaneId,
        fault: AttachFault,
    },
    /// The host's listening ports; the whole list.
    Ports {
        host: HostAlias,
        generation: Generation,
        ports: Vec<ListeningPort>,
    },
    /// A session's program ended.
    Exited {
        host: HostAlias,
        generation: Generation,
        session: SessionId,
    },
    /// The connection ended.
    Closed {
        host: HostAlias,
        generation: Generation,
    },
    /// The `ssh` that made the shared connection (and asked for passwords in the pane) ended
    /// successfully: the connection is up.
    Connected { pane: PaneId },
}

/// What a pane's session starts from.
#[derive(Debug, Clone)]
pub enum Wanted {
    /// A new session in `cwd` with `env` as its whole environment.
    Fresh {
        cwd: String,
        env: Vec<(String, String)>,
    },
    /// The session the pane had before the link dropped.
    Again(SessionId),
}

/// A pane to open or reattach.
#[derive(Debug)]
pub struct Ticket {
    pub pane: PaneId,
    pub target: SharedTarget,
    pub feeder: Feeder,
    pub wanted: Wanted,
}

/// Starts jobs. Cloning shares the channel.
#[derive(Debug, Clone)]
pub struct RemoteWork {
    events: UnboundedSender<RemoteEvent>,
}

impl RemoteWork {
    pub fn new(events: UnboundedSender<RemoteEvent>) -> Self {
        Self { events }
    }

    /// The channel for events that come from threads of the window's own making (the pane's
    /// first connection).
    pub fn sender(&self) -> UnboundedSender<RemoteEvent> {
        self.events.clone()
    }

    /// Runs `job` on a thread. If the thread cannot start, `failed` is sent instead.
    fn run(
        &self,
        job: impl FnOnce(&UnboundedSender<RemoteEvent>) + Send + 'static,
        failed: Option<RemoteEvent>,
    ) {
        let events = self.events.clone();
        let started = std::thread::Builder::new()
            .name("temor-remote-job".to_owned())
            .spawn(move || job(&events));
        if let (Err(_), Some(failed)) = (started, failed) {
            let _ = self.events.send(failed);
        }
    }

    /// Asks what `host` has, when it can run the helper.
    pub fn probe(&self, env: &SshEnv, host: HostAlias, wanted: HelperHash) {
        let (env, none) = (env.clone(), probed(&host, None));
        self.run(
            move |events| {
                let probe = env.layout.clone().ok().and_then(|layout| {
                    let argv = command_argv(&host, &layout, PROBE_SCRIPT);
                    let out = env.runner.run(&env.tools.ssh, &argv).ok()?;
                    (out.code == Some(0)).then(|| {
                        let text = String::from_utf8_lossy(&out.stdout);
                        parse_probe(&text, std::env::consts::ARCH, &wanted)
                    })
                });
                let _ = events.send(probed(&host, probe));
            },
            Some(none),
        );
    }

    /// Stores the helper on `host` and reports the hash of what is there.
    pub fn install(&self, env: &SshEnv, host: HostAlias, blob: HelperBlob) {
        let env = env.clone();
        let none = RemoteEvent::Installed {
            host: host.clone(),
            hash: None,
        };
        self.run(
            move |events| {
                let hash = env.layout.clone().ok().and_then(|layout| {
                    let argv = command_argv(&host, &layout, &install_script(&blob.hash));
                    let out = run_with_input(&env, &argv, &blob.bytes).ok()?;
                    let said = String::from_utf8_lossy(&out.stdout)
                        .trim()
                        .to_ascii_lowercase();
                    let valid = out.code == Some(0) && ContentHash::from_hex(&said).is_some();
                    valid.then_some(HelperHash(said))
                });
                let _ = events.send(RemoteEvent::Installed { host, hash });
            },
            Some(none),
        );
    }

    /// Removes an old build from `host`. Its outcome does not matter.
    pub fn remove(&self, env: &SshEnv, host: HostAlias, hash: ContentHash) {
        let env = env.clone();
        self.run(
            move |_| {
                if let Ok(layout) = env.layout.clone() {
                    let argv = command_argv(&host, &layout, &remove_script(&hash));
                    // A build that stays is only wasted space.
                    let _ = env.runner.run(&env.tools.ssh, &argv);
                }
            },
            None,
        );
    }

    /// Connects to `host`'s helper (build `hash`) and starts reading its events into `router`.
    pub fn dial(
        &self,
        env: &SshEnv,
        host: HostAlias,
        generation: Generation,
        hash: ContentHash,
        router: Router,
    ) {
        let env = env.clone();
        let failed = RemoteEvent::Dialed {
            host: host.clone(),
            generation,
            outcome: Err(DialFault::Failed),
        };
        self.run(
            move |events| {
                let Ok(layout) = env.layout.clone() else {
                    let outcome = Err(DialFault::Failed);
                    let _ = events.send(RemoteEvent::Dialed {
                        host,
                        generation,
                        outcome,
                    });
                    return;
                };
                let argv = dial_argv(&host, &layout, &hash);
                let outcome = dial(&env.tools.ssh, &argv, hash).map(|(conn, incoming)| {
                    let (tell, name) = (events.clone(), host.clone());
                    let reader = std::thread::Builder::new()
                        .name("temor-remote-events".to_owned())
                        .spawn(move || pump(&name, generation, &incoming, &router, &tell));
                    (conn, reader)
                });
                let outcome = match outcome {
                    Ok((conn, Ok(_))) => {
                        if let Ok(list) = ports(conn.client()) {
                            let _ = events.send(RemoteEvent::Ports {
                                host: host.clone(),
                                generation,
                                ports: list,
                            });
                        }
                        Ok(conn)
                    }
                    Ok((_, Err(_))) => Err(DialFault::Failed),
                    Err(fault) => Err(fault),
                };
                let _ = events.send(RemoteEvent::Dialed {
                    host,
                    generation,
                    outcome,
                });
            },
            Some(failed),
        );
    }

    /// Opens or reattaches a pane's session on `conn` and hands the pane its screen.
    pub fn attach(
        &self,
        conn: Arc<Conn>,
        router: Router,
        (host, generation): (HostAlias, Generation),
        ticket: Ticket,
    ) {
        let pane = ticket.pane;
        let failed = RemoteEvent::AttachFailed {
            host: host.clone(),
            generation,
            pane,
            fault: AttachFault::Failed,
        };
        self.run(
            move |events| {
                let event = attach_pane(&conn, &router, ticket)
                    .map(|session| RemoteEvent::Attached {
                        host: host.clone(),
                        generation,
                        pane,
                        session,
                    })
                    .unwrap_or_else(|fault| RemoteEvent::AttachFailed {
                        host,
                        generation,
                        pane,
                        fault,
                    });
                let _ = events.send(event);
            },
            Some(failed),
        );
    }

    /// Ends a pane's session on the host. Its outcome does not matter.
    pub fn close(&self, conn: Arc<Conn>, session: SessionId) {
        self.run(
            move |_| {
                // The session may already be gone with its link.
                let _ = close(conn.client(), session);
            },
            None,
        );
    }
}

fn probed(host: &HostAlias, probe: Option<Probe>) -> RemoteEvent {
    RemoteEvent::Probed {
        host: host.clone(),
        probe,
    }
}

fn attach_pane(
    conn: &Arc<Conn>,
    router: &Router,
    ticket: Ticket,
) -> Result<SessionId, AttachFault> {
    let client = conn.client();
    let size = lock(&ticket.target).size;
    let opened = match &ticket.wanted {
        Wanted::Fresh { cwd, env } => open(client, cwd, env, size),
        Wanted::Again(session) => Ok(*session),
    };
    let attached = opened.and_then(|session| {
        // The screen the host sends back is at the size the window has now.
        let _ = resize(client, session, size);
        attach(client, session).map(|shown| (session, shown))
    });
    match attached {
        Ok((session, shown)) => {
            router.attach(session, ticket.feeder, shown.screen());
            let mut target = lock(&ticket.target);
            target.conn = Some(Arc::clone(conn));
            target.session = Some(session);
            Ok(session)
        }
        Err(Error::Remote(RemoteError::NoSuchSession { .. })) => Err(AttachFault::Gone),
        Err(_) => Err(AttachFault::Failed),
    }
}

/// Runs `ssh` with `input` on its stdin and waits for it.
fn run_with_input(env: &SshEnv, argv: &[String], input: &[u8]) -> std::io::Result<CommandOutput> {
    use std::io::Write;
    let mut child = Command::new(&env.tools.ssh)
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // A host that stops reading shows up as a failed exit below.
        let _ = stdin.write_all(input);
    }
    let out = child.wait_with_output()?;
    Ok(CommandOutput {
        code: out.status.code(),
        stdout: out.stdout,
        stderr: out.stderr,
    })
}

/// The exit of a connect-phase `ssh`, as the window's event stream reports it.
pub fn connect_succeeded(exit: Option<ChildExit>) -> bool {
    exit == Some(ChildExit::Exited(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_clean_exit_means_the_shared_connection_is_up() {
        let cases = [
            (Some(ChildExit::Exited(0)), true),
            (Some(ChildExit::Exited(255)), false),
            (Some(ChildExit::Signaled(9)), false),
            (None, false),
        ];
        for (exit, want) in cases {
            assert_eq!(connect_succeeded(exit), want, "{exit:?}");
        }
    }
}

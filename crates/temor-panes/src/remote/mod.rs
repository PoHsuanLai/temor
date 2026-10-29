//! The jobs behind a Tier B session that wait on `ssh` or the network (PLAN §9b): finding out what
//! a host has, putting the helper there, connecting to it, and opening or reattaching a pane's
//! session. Each runs on a thread of its own and reports as a [`RemoteEvent`], which the hub
//! reads on the window's thread the way it reads a pane's events. The decisions are the pure
//! steps in temor-core and temor-wire; these only run what they name.

pub mod attach;
pub mod event;
pub mod link;
pub mod palette;
pub mod pump;
pub mod relay;
pub mod route;

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use temor_core::vocab::text::HelperHash;
use temor_remote::ClientEvent;
use temor_remote::host::HostEnv;
use temor_ssh::{CommandOutput, CommandRunner, HostAlias};
use temor_wire::digest::ContentHash;
use temor_wire::ids::SessionId;
use temor_wire::link::PingPolicy;
use temor_wire::message::Palette;
use tokio::sync::mpsc::UnboundedSender;

use crate::remote::attach::attach_pane;
use crate::remote::event::{AttachFault, Generation, RemoteEvent, Ticket};
use crate::remote::pump::pump;
use crate::remote::route::Router;
use temor_client::remote::calls::{close, ports, set_palette};
use temor_client::remote::conn::{Conn, DialFault, dial, dial_argv, dial_local};
use temor_client::remote::install::{
    Found, HelperBlob, PROBE_SCRIPT, command_argv, install_script, parse_probe, remove_script,
};
use temor_client::remote::ping::watch;
use temor_client::ssh::SshEnv;

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
    pub fn probe(&self, env: &SshEnv, host: HostAlias, ours: Vec<HelperHash>) {
        let (env, none) = (env.clone(), probed(&host, None));
        self.run(
            move |events| {
                let found = env.layout.clone().ok().and_then(|layout| {
                    let argv = command_argv(&host, &layout, PROBE_SCRIPT);
                    let out = env.runner.run(&env.tools.ssh, &argv).ok()?;
                    (out.code == Some(0)).then(|| {
                        let text = String::from_utf8_lossy(&out.stdout);
                        parse_probe(&text, &ours)
                    })
                });
                let _ = events.send(probed(&host, found));
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
                let dialed = dial(&env.tools.ssh, &argv, hash);
                serve(events, host, generation, router, dialed);
            },
            Some(failed),
        );
    }

    /// Connects to this computer's own daemon (starting it with `helper`) and starts reading its
    /// events into `router`. `host` is the name the window gives it.
    pub fn dial_local(
        &self,
        env: HostEnv,
        host: HostAlias,
        generation: Generation,
        router: Router,
    ) {
        let failed = RemoteEvent::Dialed {
            host: host.clone(),
            generation,
            outcome: Err(DialFault::Failed),
        };
        self.run(
            move |events| {
                let dialed = dial_local(&env);
                serve(events, host, generation, router, dialed);
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

    /// Gives the sessions `sessions` of `conn` the window's new colours. Its outcome does not
    /// matter: the next attach sends them again.
    pub fn palette(&self, conn: Arc<Conn>, sessions: Vec<SessionId>, palette: Palette) {
        self.run(
            move |_| {
                for session in sessions {
                    let _ = set_palette(conn.client(), session, palette);
                }
            },
            None,
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

/// What follows a dial that connected: the thread that reads the connection's events, the watch
/// on its pings, and its listening ports. Then the window is told how the dial went.
fn serve(
    events: &UnboundedSender<RemoteEvent>,
    host: HostAlias,
    generation: Generation,
    router: Router,
    dialed: Result<(Arc<Conn>, Receiver<ClientEvent>), DialFault>,
) {
    let outcome = dialed.and_then(|(conn, incoming)| {
        let (tell, name) = (events.clone(), host.clone());
        std::thread::Builder::new()
            .name("temor-remote-events".to_owned())
            .spawn(move || pump(&name, generation, &incoming, &router, &tell))
            .map_err(|_| DialFault::Failed)?;
        let stalled = RemoteEvent::Stalled {
            host: host.clone(),
            generation,
        };
        let tell = events.clone();
        // Without the watch a stall goes unnoticed; the keepalives still run.
        let _ = watch(&conn, PingPolicy::DEFAULT, move || {
            let _ = tell.send(stalled);
        });
        if let Ok(list) = ports(conn.client()) {
            let _ = events.send(RemoteEvent::Ports {
                host: host.clone(),
                generation,
                ports: list,
            });
        }
        Ok(conn)
    });
    let _ = events.send(RemoteEvent::Dialed {
        host,
        generation,
        outcome,
    });
}

fn probed(host: &HostAlias, found: Option<Found>) -> RemoteEvent {
    RemoteEvent::Probed {
        host: host.clone(),
        found,
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

//! A live connection to a host's helper: the `ssh` process that carries it and the client that
//! speaks over its stdio (PLAN §9b). Dropping the connection ends the `ssh`.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use temor_remote::host::HostEnv;
use temor_remote::{Client, ClientEvent, DAEMON_CAPS, Error, hello};
use temor_ssh::{ControlLayout, HostAlias};
use temor_wire::digest::ContentHash;
use temor_wire::hello::HelperVersion;

use crate::remote::install::helper_path;

/// Why a connection could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialFault {
    /// `ssh` did not start.
    Spawn(String),
    /// The helper there speaks nothing this temor does: trying again meets the same refusal.
    Refused,
    /// The connection or the greeting failed; another try may work.
    Failed,
}

/// A connection. `Send` and `Sync`; requests from several threads take turns.
pub struct Conn {
    client: Client,
    child: Mutex<Option<Child>>,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn").finish_non_exhaustive()
    }
}

impl Conn {
    /// A connection over a client that is already connected and has no `ssh` of its own (a link
    /// made some other way; the tests' pipes).
    pub fn bare(client: Client) -> Self {
        Self {
            client,
            child: Mutex::new(None),
        }
    }

    /// Ends the `ssh` that carries this connection, so a request stuck on a stalled link fails
    /// and the helper's side sees the connection go. The connection is no use afterwards.
    pub fn drop_link(&self) {
        let child = self.child.lock().ok().and_then(|mut slot| slot.take());
        if let Some(child) = child {
            reap(child);
        }
    }

    /// The client, for requests.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        let child = self.child.get_mut().ok().and_then(|slot| slot.take());
        if let Some(mut child) = child {
            // Already gone is fine: the wait below reaps it either way.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The `ssh` arguments that run the helper's `stdio` bridge on `alias`: no terminal, never
/// asking for a password (the shared connection is already up, or the dial fails), joining the
/// shared connection, and noticing a link that stalled.
pub fn dial_argv(alias: &HostAlias, layout: &ControlLayout, hash: &ContentHash) -> Vec<String> {
    let mut argv: Vec<String> = ["-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
    // The wire's ping (remote_ping) is the stall detector; these keepalives stay as a second one.
    for option in [
        "ServerAliveInterval=15".to_owned(),
        "ServerAliveCountMax=3".to_owned(),
        "ControlMaster=auto".to_owned(),
        layout.control_path_option(),
        layout.control_persist_option(),
    ] {
        argv.push("-o".to_owned());
        argv.push(option);
    }
    argv.extend([
        "--".to_owned(),
        alias.as_str().to_owned(),
        format!("{} stdio", helper_path(hash)),
    ]);
    argv
}

/// A connection to this computer's own daemon, started when none is running. `env.exe` is the
/// helper to start.
pub fn dial_local(env: &HostEnv) -> Result<(Arc<Conn>, Receiver<ClientEvent>), DialFault> {
    match temor_remote::host::dial_local(env) {
        Ok((client, events)) => Ok((Arc::new(Conn::bare(client)), events)),
        Err(Error::Incompatible(_)) => Err(DialFault::Refused),
        Err(_) => Err(DialFault::Failed),
    }
}

/// Starts `ssh` with `argv` and greets the helper over it, expecting the build `hash`.
pub fn dial(
    ssh: &Path,
    argv: &[String],
    hash: ContentHash,
) -> Result<(Arc<Conn>, Receiver<ClientEvent>), DialFault> {
    let mut child = Command::new(ssh)
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| DialFault::Spawn(err.to_string()))?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        reap(child);
        return Err(DialFault::Failed);
    };
    match Client::connect(stdout, stdin, hello(HelperVersion(hash), &DAEMON_CAPS)) {
        Ok((client, events)) => {
            let conn = Conn {
                client,
                child: Mutex::new(Some(child)),
            };
            Ok((Arc::new(conn), events))
        }
        Err(error) => {
            reap(child);
            Err(match error {
                Error::Incompatible(_) => DialFault::Refused,
                _ => DialFault::Failed,
            })
        }
    }
}

fn reap(mut child: Child) {
    // Already gone is fine.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_ssh::{ControlPersist, RuntimeDir, Seconds};

    #[test]
    fn the_dial_joins_the_shared_connection_and_never_asks_for_a_password() {
        let runtime = RuntimeDir::new(std::path::Path::new("/run/user/1000/temor"))
            .expect("an absolute folder");
        let persist = ControlPersist::Idle(Seconds::new(600).expect("positive"));
        let layout = ControlLayout::new(runtime, persist).expect("a short path");
        let alias = HostAlias::new("prod").expect("a name");
        let hash = ContentHash([0xab; 32]);
        let argv = dial_argv(&alias, &layout, &hash);
        assert!(argv.windows(2).any(|w| w == ["-o", "BatchMode=yes"]));
        assert!(argv.iter().any(|a| a.starts_with("ControlPath=")));
        let tail: Vec<&str> = argv.iter().rev().take(3).map(String::as_str).collect();
        assert_eq!(tail[2], "--", "the destination follows --");
        assert_eq!(tail[1], "prod");
        assert!(
            tail[0].ends_with("/temor-remote stdio") && tail[0].contains(&hash.to_hex()),
            "{tail:?}"
        );
    }
}

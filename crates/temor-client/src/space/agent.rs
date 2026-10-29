//! A Space's own ssh-agent (PLAN §7): started on the Space's first new shell, holding only the
//! Space's key, listening at the socket `temor_core::space::agent_plan` chose
//! (`<runtime>/temor/agent-<id>.sock`, in a folder only this user can use). The agent runs in
//! the foreground as temor's child and ends, with its socket, when it is dropped.
// TODO(platform:windows): the agent is a named pipe there; this edge does not start one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ds_style::space::list::SpaceId;
use temor_core::space::AgentPlan;
use temor_core::vocab::home::expand_home;

use crate::keep::private::ensure_private_dir;

/// How long the agent has to make its socket.
const SOCKET_WAIT: Duration = Duration::from_secs(2);

/// One running agent.
#[derive(Debug)]
pub struct RunningAgent {
    child: Child,
    socket: PathBuf,
}

impl RunningAgent {
    /// Whether the agent is still there to talk to.
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None)) && self.socket.exists()
    }
}

impl Drop for RunningAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Why a Space's agent could not start, in words for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the Space's key agent could not start: {0}")]
pub struct AgentFault(pub String);

fn fault(what: impl std::fmt::Display) -> AgentFault {
    AgentFault(what.to_string())
}

/// Starts the agent of `plan` and loads its key (`~/` in the key's path is `home`).
pub fn start(plan: &AgentPlan, home: &Path) -> Result<RunningAgent, AgentFault> {
    let socket = PathBuf::from(&plan.socket);
    let folder = socket
        .parent()
        .ok_or_else(|| fault("the socket has no folder"))?;
    ensure_private_dir(folder).map_err(fault)?;
    // A socket left by a temor that did not get to clean up.
    let _ = std::fs::remove_file(&socket);
    let child = Command::new("ssh-agent")
        .arg("-D")
        .arg("-a")
        .arg(&socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(fault)?;
    let mut agent = RunningAgent { child, socket };
    let begun = Instant::now();
    while !agent.socket.exists() {
        if !matches!(agent.child.try_wait(), Ok(None)) || begun.elapsed() > SOCKET_WAIT {
            return Err(fault("it did not start"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let key = PathBuf::from(expand_home(plan.identity.as_str(), &home.to_string_lossy()));
    let added = Command::new("ssh-add")
        .arg(&key)
        .env("SSH_AUTH_SOCK", &agent.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(fault)?;
    match added.success() {
        true => Ok(agent),
        false => Err(fault("the key could not be added")),
    }
}

/// The agents of the Spaces, one each.
#[derive(Debug, Default)]
pub struct SpaceAgents {
    running: BTreeMap<SpaceId, RunningAgent>,
}

impl SpaceAgents {
    /// Makes sure `space` has its agent running, starting it when it has none or it has died.
    pub fn ensure(
        &mut self,
        space: SpaceId,
        plan: &AgentPlan,
        home: &Path,
    ) -> Result<(), AgentFault> {
        if self
            .running
            .get_mut(&space)
            .is_some_and(RunningAgent::alive)
        {
            return Ok(());
        }
        self.running.remove(&space);
        let agent = start(plan, home)?;
        self.running.insert(space, agent);
        Ok(())
    }

    /// Ends the agent of a Space that was deleted.
    pub fn stop(&mut self, space: SpaceId) {
        self.running.remove(&space);
    }
}

//! A Space's own ssh-agent, as a plan (PLAN §7): which key it holds and where its socket is.
//! The plan is data; the edge starts the agent, creates the socket's folder (mode 0700), loads
//! the key and removes the socket when the Space's last shell ends.

use super::project::Profile;
use crate::error::CoreError;
use crate::vocab::text::{EnvKey, EnvValue, KeyPath};
use ds_style::space::list::SpaceId;

/// The most bytes a Unix socket path may have (`sun_path` is 108 with its final NUL).
const MOST_SOCKET_BYTES: usize = 107;

/// The person's runtime folder (`$XDG_RUNTIME_DIR`, such as `/run/user/1000`), given by the
/// edge: the core reads no environment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeDir(pub String);

/// What a Space's agent is: the key it holds and the socket shells reach it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPlan {
    /// The only key the agent holds.
    pub identity: KeyPath,
    /// Where it listens.
    pub socket: String,
    /// The variable new shells get so `ssh` finds it: `SSH_AUTH_SOCK` set to `socket`.
    pub env: (EnvKey, EnvValue),
}

/// The agent `space` needs, or none when its profile names no key. The socket is
/// `<runtime>/temor/agent-<space id>.sock`: one per Space, under temor's own folder.
// TODO(platform:macos) and TODO(platform:windows): the runtime folder and the socket kind
// differ there; the edge supplies a `RuntimeDir` and this stays text.
pub fn agent_plan(
    space: SpaceId,
    profile: &Profile,
    runtime: &RuntimeDir,
) -> Result<Option<AgentPlan>, CoreError> {
    let Some(identity) = profile.ssh_identity.clone() else {
        return Ok(None);
    };
    let base = runtime.0.trim_end_matches('/');
    let socket = format!("{base}/temor/agent-{}.sock", space.0);
    if socket.len() > MOST_SOCKET_BYTES {
        return Err(CoreError::SocketPathTooLong {
            len: socket.len(),
            max: MOST_SOCKET_BYTES,
        });
    }
    let env = (EnvKey::new("SSH_AUTH_SOCK")?, EnvValue(socket.clone()));
    Ok(Some(AgentPlan {
        identity,
        socket,
        env,
    }))
}

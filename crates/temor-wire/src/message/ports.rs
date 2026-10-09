//! Listening ports the helper finds among the person's sessions.

use crate::units::{Pid, Port};
use serde::{Deserialize, Serialize};

/// One TCP port a process on the host is listening on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ListeningPort {
    /// The port.
    pub port: Port,
    /// The process that owns the listening socket.
    pub pid: Pid,
    /// The process's name, as the host shows it.
    pub process: String,
}

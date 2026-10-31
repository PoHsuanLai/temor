//! Identifiers. Each is a newtype, so an id can never be passed where another is wanted.
//!
//! The client chooses [`RequestId`]s and the helper chooses the rest. Ids are never reused on
//! one connection; a new connection starts a new id space.

use serde::{Deserialize, Serialize};

/// Names one request, so its reply (or error) can be matched to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub u64);

/// One remote session: a shell (or other program) on the host with its own screen state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteSessionId(pub u64);

/// One file watch, named by the helper when it is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WatchId(pub u64);

/// One terminal started through the agent relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerminalId(pub u64);

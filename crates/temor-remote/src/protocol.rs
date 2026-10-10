//! What this build speaks: the protocol version and the greeting.

use temor_wire::hello::{Capability, Hello, HelperVersion, ProtocolVersion};

/// The one protocol version this build speaks. Version 2 added streamed writes, `Ping`, the cell
/// size on open and resize, and the palette. This build refuses a version 1 peer, which cannot
/// do them: a helper at another version is replaced by the hash check before it matters.
pub const PROTOCOL: ProtocolVersion = ProtocolVersion(2);

/// The capabilities the daemon offers. Trial runs and the agent relay need the sandbox runner
/// (bulkhead), which is not linked yet, so they are not offered.
///
/// `Facts` (a session's folder read for the chips) was added without a new protocol version: a
/// request and a reply nobody sends unless both ends agreed the capability change nothing for an
/// older peer, which never offers it and is never asked.
pub const DAEMON_CAPS: [Capability; 4] = [
    Capability::Sessions,
    Capability::Ports,
    Capability::Files,
    Capability::Facts,
];

/// A greeting for this build, carrying `helper` as the identity of the helper binary (the
/// daemon's own hash, or the hash temor expects of the host).
pub fn hello(helper: HelperVersion, caps: &[Capability]) -> Hello {
    Hello {
        protocol: PROTOCOL,
        oldest_protocol: PROTOCOL,
        helper,
        caps: caps.to_vec(),
    }
}

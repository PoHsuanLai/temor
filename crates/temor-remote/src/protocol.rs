//! What this build speaks: the protocol version and the greeting.

use temor_wire::hello::{Capability, Hello, HelperVersion, ProtocolVersion};

/// The one protocol version this build speaks.
pub const PROTOCOL: ProtocolVersion = ProtocolVersion(1);

/// The capabilities the daemon offers. Trial runs and the agent relay need the sandbox runner
/// (bulkhead), which is not linked yet, so they are not offered.
pub const DAEMON_CAPS: [Capability; 3] =
    [Capability::Sessions, Capability::Ports, Capability::Files];

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

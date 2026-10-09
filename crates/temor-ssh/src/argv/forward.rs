//! Local port forwards for `ssh -O forward` on a shared connection: `-L [bind:]port:host:port`.

use crate::config::Port;
use crate::error::SshError;

/// A bind address or remote host for a forward: letters, digits, `.`, `-`, `_` and `:`
/// (IPv6 literals are bracketed in the spec).
fn host_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}

/// Where a forward listens locally: an address such as `127.0.0.1`, or `localhost` by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindAddress(String);

impl BindAddress {
    /// Validates the address.
    pub fn new(text: &str) -> Result<Self, SshError> {
        match host_text(text) {
            true => Ok(Self(text.to_owned())),
            false => Err(SshError::InvalidForward(text.to_owned())),
        }
    }
}

/// The host the remote end connects to, as seen from the host (`localhost` for the host's own
/// ports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteHost(String);

impl RemoteHost {
    /// Validates the host.
    pub fn new(text: &str) -> Result<Self, SshError> {
        match host_text(text) {
            true => Ok(Self(text.to_owned())),
            false => Err(SshError::InvalidForward(text.to_owned())),
        }
    }
}

/// One local forward: local `bind:local` to `remote_host:remote_port` on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forward {
    pub bind: Option<BindAddress>,
    pub local: Port,
    pub remote_host: RemoteHost,
    pub remote: Port,
}

/// The `-L` value of a forward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardSpec(String);

impl ForwardSpec {
    /// The text for `-L`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Forward {
    /// `[bind:]local:host:remote`, with IPv6 hosts bracketed.
    pub fn spec(&self) -> ForwardSpec {
        let host = bracketed(&self.remote_host.0);
        let text = match &self.bind {
            Some(bind) => format!(
                "{}:{}:{}:{}",
                bracketed(&bind.0),
                self.local.get(),
                host,
                self.remote.get()
            ),
            None => format!("{}:{}:{}", self.local.get(), host, self.remote.get()),
        };
        ForwardSpec(text)
    }
}

fn bracketed(host: &str) -> String {
    match host.contains(':') {
        true => format!("[{host}]"),
        false => host.to_owned(),
    }
}

//! Values from the config checked before they become `ssh` arguments: an identity file for
//! `-i`, a jump list for `-J`, and the hint for an IPv6 address typed as a host. The config
//! types keep what was written; nothing here changes them.

use std::net::Ipv6Addr;
use std::path::Path;

use super::model::{IdentityFile, ProxyJump};
use crate::error::SshError;

/// The longest jump hop `ssh` is handed.
const MAX_HOP_BYTES: usize = 255;

impl IdentityFile {
    /// The key file as the argument after `ssh -i`: an absolute path (a leading `~` expanded
    /// with `home`), not starting with `-`, which `ssh` would read as an option.
    pub fn for_argv(&self, home: Option<&Path>) -> Result<String, SshError> {
        let unsafe_value = || SshError::UnsafeValue(self.as_written().to_owned());
        let path = self.expanded(home).ok_or_else(unsafe_value)?;
        let text = path.to_str().ok_or_else(unsafe_value)?;
        match path.is_absolute() && !text.starts_with('-') {
            true => Ok(text.to_owned()),
            false => Err(unsafe_value()),
        }
    }
}

impl ProxyJump {
    /// The jump list as the argument after `ssh -J`: each hop `[user@]host[:port]` (or an
    /// `ssh://` form) with only the characters a host name may have, none starting with `-`.
    pub fn for_argv(&self) -> Result<&str, SshError> {
        let text = self.as_str();
        match text.split(',').all(hop_is_plain) {
            true => Ok(text),
            false => Err(SshError::UnsafeValue(text.to_owned())),
        }
    }
}

fn hop_is_plain(hop: &str) -> bool {
    let bare = hop.strip_prefix("ssh://").unwrap_or(hop);
    !bare.is_empty()
        && bare.len() <= MAX_HOP_BYTES
        && !bare.starts_with('-')
        && bare.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '-' | '_' | '@' | ':' | '[' | ']' | '%' | '+')
        })
}

/// Whether `text` is an IPv6 address, bare or in brackets. A host name here cannot hold one:
/// `ssh_config` needs a `Host` with the address as its `HostName`.
pub fn is_ipv6_literal(text: &str) -> bool {
    let inner = text.trim();
    let inner = inner
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(inner);
    let inner = inner.rsplit_once('@').map_or(inner, |(_, host)| host);
    inner.parse::<Ipv6Addr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_file_goes_to_dash_i_only_as_an_absolute_path_that_is_not_an_option() {
        let home = Path::new("/home/me");
        let cases = [
            (
                "~/.ssh/id_ed25519",
                Some(home),
                Some("/home/me/.ssh/id_ed25519"),
            ),
            ("/keys/id", None, Some("/keys/id")),
            ("~/.ssh/id", None, None),
            ("-oProxyCommand=x", Some(home), None),
            ("relative/id", Some(home), None),
            ("~", Some(home), Some("/home/me")),
        ];
        for (written, home, want) in cases {
            let file = IdentityFile::new(written).expect("usable text");
            assert_eq!(file.for_argv(home).ok().as_deref(), want, "{written}");
        }
    }

    #[test]
    fn a_jump_list_goes_to_dash_j_only_when_every_hop_is_a_plain_host() {
        let cases = [
            ("bastion", true),
            ("me@bastion:2222,inner", true),
            ("ssh://me@bastion:22", true),
            ("[::1]:22", true),
            ("-oProxyCommand=x", false),
            ("a,-b", false),
            ("a,,b", false),
            ("a b", false),
            ("a;b", false),
            ("a$(x)", false),
        ];
        for (written, ok) in cases {
            let jump = ProxyJump::new(written).expect("usable text");
            assert_eq!(jump.for_argv().is_ok(), ok, "{written}");
        }
    }

    #[test]
    fn an_ipv6_address_is_known_with_or_without_brackets_and_a_user() {
        let cases = [
            ("::1", true),
            ("[2001:db8::1]", true),
            ("me@fe80::1", true),
            ("prod", false),
            ("10.0.0.1", false),
            ("host:22", false),
        ];
        for (text, want) in cases {
            assert_eq!(is_ipv6_literal(text), want, "{text}");
        }
    }
}

//! Which name `known_hosts` records a host under. An alias in the person's ssh config is not
//! that name: `ssh -G` prints the `HostName` and port ssh would connect to (and the
//! `HostKeyAlias`, which replaces the name), and that is what `ssh-keygen -F` must be asked for.

use crate::config::{HostAlias, HostName, Port};

/// The name and port a host's key is recorded under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownName {
    pub host: HostName,
    pub port: Option<Port>,
}

/// `ssh -G <alias>`: prints the settings ssh would use for the alias. It never connects.
pub fn resolve_argv(alias: &HostAlias) -> Vec<String> {
    vec!["-G".to_owned(), alias.as_str().to_owned()]
}

/// The name and port from `ssh -G` output: the `hostkeyalias` when the config sets one, else
/// the `hostname`. `None` when the output names neither.
pub fn known_name_from_resolved(output: &str) -> Option<KnownName> {
    let (mut hostname, mut alias, mut port) = (None, None, None);
    for line in output.lines() {
        let Some((key, value)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        let value = value.trim();
        match key.to_ascii_lowercase().as_str() {
            "hostname" => hostname = HostName::new(value),
            "hostkeyalias" => alias = HostName::new(value),
            "port" => port = Port::parse(value),
            _ => {}
        }
    }
    alias.or(hostname).map(|host| KnownName { host, port })
}

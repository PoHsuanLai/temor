//! Blocks into host rows: every concrete `Host` name, with the settings OpenSSH would use for
//! it.

use super::include::{ConfigFiles, ConfigPaths};
use super::model::{ConfigIssue, HostAlias, HostEntry, HostList, HostName, IssueReason};
use super::parse::{Block, Setting, Walker};
use super::pattern::Scope;

/// Loads `paths.config` and everything it includes, and lists its concrete hosts in the order
/// they first appear. Never fails: unreadable parts become issues.
pub fn load_host_list(files: &dyn ConfigFiles, paths: &ConfigPaths) -> HostList {
    let mut walker = Walker::new(files, paths);
    walker.load_file(&paths.config, 0);
    let (blocks, mut issues) = walker.into_parts();
    let aliases = collect_aliases(&blocks, &mut issues);
    let entries = aliases
        .iter()
        .map(|alias| entry_for(alias, &blocks))
        .collect();
    HostList { entries, issues }
}

/// Concrete names from the `Host` lines, deduplicated case-insensitively in first-seen order.
fn collect_aliases(blocks: &[Block], issues: &mut Vec<ConfigIssue>) -> Vec<HostAlias> {
    let mut aliases: Vec<HostAlias> = Vec::new();
    for block in blocks {
        let Scope::Hosts(patterns) = &block.scope else {
            continue;
        };
        for name in patterns.iter().filter_map(|pattern| pattern.concrete()) {
            match HostAlias::new(name) {
                Ok(alias) => {
                    let seen = aliases
                        .iter()
                        .any(|known| known.as_str().eq_ignore_ascii_case(alias.as_str()));
                    if !seen {
                        aliases.push(alias);
                    }
                }
                Err(_) => issues.push(ConfigIssue {
                    file: block.origin.file.clone(),
                    line: block.origin.line,
                    reason: IssueReason::UnsafeAlias {
                        alias: name.to_owned(),
                    },
                }),
            }
        }
    }
    aliases
}

/// OpenSSH's rule: the first value obtained for a keyword wins, while `IdentityFile` lines
/// from every matching block accumulate.
fn entry_for(alias: &HostAlias, blocks: &[Block]) -> HostEntry {
    let mut entry = HostEntry {
        alias: alias.clone(),
        hostname: HostName::from_alias(alias),
        user: None,
        port: None,
        identity_files: Vec::new(),
        proxy_jump: None,
    };
    let mut hostname = None;
    for block in blocks.iter().filter(|b| b.scope.matches(alias.as_str())) {
        for setting in &block.settings {
            match setting {
                Setting::HostName(v) => keep_first(&mut hostname, v),
                Setting::User(v) => keep_first(&mut entry.user, v),
                Setting::Port(v) => keep_first(&mut entry.port, v),
                Setting::ProxyJump(v) => keep_first(&mut entry.proxy_jump, v),
                Setting::IdentityFile(v) => entry.identity_files.push(v.clone()),
            }
        }
    }
    if let Some(name) = hostname {
        entry.hostname = name;
    }
    entry
}

fn keep_first<T: Clone>(slot: &mut Option<T>, value: &T) {
    if slot.is_none() {
        *slot = Some(value.clone());
    }
}

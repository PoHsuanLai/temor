//! `~/.ssh/config` into host rows (PLAN §9b): lenient parsing, `Include` with globs, `Host`
//! patterns with `!` negation, and `Match` blocks skipped with a note.

mod glob;
mod include;
mod known_hosts;
mod lex;
mod model;
mod parse;
mod pattern;
mod resolve;
#[cfg(test)]
mod tests;

pub use include::{ConfigFiles, ConfigPaths};
pub use known_hosts::known_host_names;
pub use model::{
    ConfigIssue, HostAlias, HostEntry, HostList, HostName, IdentityFile, IssueReason, LineNo, Port,
    ProxyJump, UserName,
};
pub use resolve::load_host_list;

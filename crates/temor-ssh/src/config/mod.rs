//! `~/.ssh/config` into host rows (PLAN §9b): lenient parsing, `Include` with globs, `Host`
//! patterns with `!` negation, and `Match` blocks skipped with a note.

mod fs;
mod glob;
mod include;
mod known_hosts;
mod lex;
mod model;
mod parse;
mod pattern;
mod resolve;
#[cfg(feature = "testing")]
pub mod support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_files;
#[cfg(test)]
mod tests_misc;

pub use fs::FsConfigFiles;
pub use include::{ConfigFiles, ConfigPaths};
pub use known_hosts::known_host_names;
pub use model::{
    ConfigIssue, HostAlias, HostEntry, HostList, HostName, IdentityFile, IssueReason, LineNo, Port,
    ProxyJump, UserName,
};
pub use resolve::load_host_list;

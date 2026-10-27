//! The window's SSH (PLAN §9b, Tier A): where its programs and sockets are, the host list, and
//! the jobs that wait on a program or the network, each on a thread of its own.

pub mod env;
mod hosts_cache;
pub mod work;
#[cfg(test)]
mod work_tests;

pub use env::{ArcRunner, SshEnv};

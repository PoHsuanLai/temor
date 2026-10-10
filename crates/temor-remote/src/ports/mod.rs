//! Listening-port detection (PLAN §9b, "Ports"): which TCP ports the processes of the person's
//! sessions listen on. The parsing and the joining are pure; reading `/proc` is the platform
//! edge's job.

mod assemble;
mod parse;
mod tree;

pub use assemble::{Owner, assemble};
pub use parse::{ListenSocket, parse_ppid, parse_socket_link, parse_tcp_table};
pub use tree::descendants;

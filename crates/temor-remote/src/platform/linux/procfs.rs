//! Listening ports from `/proc`. The root is injected, so tests point it at a made-up tree.

use std::fs;
use std::io;
use std::path::Path;

use temor_wire::message::ListeningPort;

use crate::error::Error;
use crate::ports::parse_tcp_table;
use crate::ports::{ListenSocket, Owner, assemble, descendants, parse_ppid, parse_socket_link};

/// The TCP ports that `roots` (session programs) and their descendants listen on, read from the
/// process filesystem at `proc`.
pub fn listening_ports(proc: &Path, roots: &[u32]) -> Result<Vec<ListeningPort>, Error> {
    let listening = listening_sockets(proc)?;
    if listening.is_empty() || roots.is_empty() {
        return Ok(Vec::new());
    }
    let tree = descendants(roots, &parent_table(proc)?);
    let owners: Vec<Owner> = tree
        .into_iter()
        .filter_map(|pid| owner(proc, pid))
        .collect();
    Ok(assemble(&listening, &owners))
}

fn listening_sockets(proc: &Path) -> Result<Vec<ListenSocket>, Error> {
    let mut sockets = Vec::new();
    for table in ["net/tcp", "net/tcp6"] {
        match fs::read_to_string(proc.join(table)) {
            Ok(text) => sockets.extend(parse_tcp_table(&text)),
            // A host without IPv6 has no tcp6 table.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::io(format!("reading {table}"), error)),
        }
    }
    Ok(sockets)
}

/// Every process as `(pid, parent pid)`. Processes that vanish while the table is read are
/// skipped.
fn parent_table(proc: &Path) -> Result<Vec<(u32, u32)>, Error> {
    let entries = fs::read_dir(proc).map_err(|error| Error::io("listing processes", error))?;
    let mut table = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(parent) = parse_ppid(&stat) {
            table.push((pid, parent));
        }
    }
    Ok(table)
}

/// A process's name and its socket inodes; `None` if it is gone or not ours to read.
fn owner(proc: &Path, pid: u32) -> Option<Owner> {
    let base = proc.join(pid.to_string());
    let name = fs::read_to_string(base.join("comm"))
        .ok()?
        .trim()
        .to_string();
    let sockets = fs::read_dir(base.join("fd"))
        .ok()?
        .flatten()
        .filter_map(|fd| fs::read_link(fd.path()).ok())
        .filter_map(|target| parse_socket_link(&target.to_string_lossy()))
        .collect();
    Some(Owner { pid, name, sockets })
}

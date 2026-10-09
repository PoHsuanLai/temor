//! temor-ssh's integration tests: the bootstrap run by a real `sh` under a scratch HOME, and the
//! argv parsed by the system `ssh` with `-G` (which never connects). No test contacts a host and
//! none reads the real `~/.ssh`. The harness uses unix APIs (symlinks, file modes), so these
//! modules exist only on unix; the library itself has no such gate.

#[cfg(unix)]
mod bootstrap_run;
#[cfg(unix)]
mod harness;
#[cfg(unix)]
mod ssh_parse;

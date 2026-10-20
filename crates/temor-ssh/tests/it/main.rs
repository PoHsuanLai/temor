//! temor-ssh's integration tests: the bootstrap run by a real `sh` under a scratch HOME, and the
//! argv parsed by the system `ssh` with `-G` (which never connects), and a real sshd on
//! 127.0.0.1 with scratch keys and a scratch HOME (`real_sshd`). None reads the real `~/.ssh`. The harness uses unix APIs (symlinks, file modes), so these
//! modules exist only on unix; the library itself has no such gate.

#[cfg(unix)]
mod bootstrap_run;
#[cfg(unix)]
mod edges;
#[cfg(unix)]
mod files;
#[cfg(unix)]
mod harness;
#[cfg(unix)]
mod inner_load;
#[cfg(unix)]
mod master;
#[cfg(unix)]
mod nu;
#[cfg(unix)]
mod real_sshd;
#[cfg(unix)]
mod shells;
#[cfg(unix)]
mod ssh_parse;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

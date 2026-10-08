//! Specs for programs started without shell integration.

use std::ffi::OsString;
use std::path::PathBuf;

use temor_pty::SpawnSpec;

use super::{Scratch, program, size};

/// `bash --norc --noprofile` with `args`, in the scratch directory with the scratch env.
pub fn bash(scratch: &Scratch, args: &[&str]) -> SpawnSpec {
    let mut all: Vec<OsString> = vec!["--norc".into(), "--noprofile".into()];
    all.extend(args.iter().map(OsString::from));
    SpawnSpec {
        program: program("bash").expect("bash on PATH"),
        args: all,
        cwd: scratch.cwd.clone(),
        env: scratch.base_env(),
        size: size(),
    }
}

/// An interactive `bash --norc` with a plain `$ ` prompt.
pub fn bash_interactive(scratch: &Scratch) -> SpawnSpec {
    let mut spec = bash(scratch, &["-i"]);
    spec.env.set("PS1", "$ ");
    spec
}

pub fn proc_alive(pid: i32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

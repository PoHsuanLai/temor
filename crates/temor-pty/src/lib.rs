//! temor's pseudo-terminals.
//!
//! This crate opens a pseudo-terminal, starts a program on it as a new session with the
//! terminal as its controlling terminal, resizes it, reads and writes it without blocking,
//! signals and reaps the child's process group, and reports the foreground process for tab
//! titles and the running row (PLAN §2, §4b, §7). It also resolves the person's shell, builds
//! the child's environment, and lays down temor's shell integration (OSC 133 prompt marks and
//! OSC 7 working directory) without touching the person's dotfiles.
//!
//! It takes plain inputs and knows nothing of temor-core or temor-vt; the app wires it up.
//! Library code never spawns threads: the caller drives [`PtyIo`] on its own I/O thread.
//! Nothing ambient is read here: the environment, the passwd lookup, the terminfo search path,
//! `/proc` and the runtime directory are all handed in.
//!
//! # Platforms
//!
//! Everything that differs by OS lives in the private `platform` module, behind its `Os`
//! trait and per-OS pseudo-terminal modules.
//! - **Linux:** everything, including the foreground process's name and working directory
//!   from `/proc`.
//! - **macOS, Windows and other OSes:** the crate compiles, [`Pty::spawn`] returns
//!   [`PtyError::Unsupported`] and [`UserId::current`] is `None`. The portable core cannot use
//!   a pseudo-terminal there yet (ConPTY on Windows is future work). The environment, shell
//!   resolution and integration planning still work everywhere.
//!
//! # Starting the session leader
//!
//! The workspace denies `unsafe`, so the child cannot call `setsid` and `TIOCSCTTY` from a
//! `pre_exec` hook. A [`SessionLauncher`] does it in a tiny exec step instead:
//! - [`SessionLauncher::Setsid`]: util-linux `setsid --ctty` (Linux).
//! - [`SessionLauncher::Trampoline`]: any program that calls [`run_session_leader`] (this
//!   crate's `temor-pty-exec`, or temor's own binary), which needs no util-linux.
//!
//! Either way the launcher `exec`s the program, so the child's pid is the program's pid, its
//! session id and its process group id.
//!
//! # The environment contract
//!
//! The child never inherits temor's environment implicitly. The caller passes a complete
//! [`ChildEnv`], usually built like this:
//!
//! 1. [`base_env`] takes an injected source (the app passes its own environment at its edge)
//!    and drops variables that describe *another* terminal: `TERM`, `COLORTERM`,
//!    `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `VTE_VERSION`, `WINDOWID`, `LINES`, `COLUMNS`, `TMUX`, `TMUX_PANE`, `STY`, every
//!    `TEMOR_*` variable except `TEMOR_SHELL_INTEGRATION`, and the `KITTY_*`, `GHOSTTY_*`,
//!    `WEZTERM_*`, `ITERM_*`, `ALACRITTY_*` and `KONSOLE_*` families.
//! 2. [`child_env`] layers temor's contract and the Space profile on top, in this order:
//!    the base, then `TERM` and `COLORTERM`, then the profile (which may override those two,
//!    for example `TERM=xterm-256color` for a host without our terminfo), then the identity
//!    variables, which the profile cannot override.
//!
//! The variables temor sets:
//!
//! | Variable | Value |
//! |---|---|
//! | `TERM` | `xterm-ghostty` when a terminfo entry for it is found, else `xterm-256color` ([`find_term`]; no `infocmp`) |
//! | `COLORTERM` | `truecolor` |
//! | `TERM_PROGRAM` | `temor` |
//! | `TERM_PROGRAM_VERSION` | the caller's [`ProgramVersion`] (temor's version) |
//! | `TEMOR_PANE_ID` | the caller's [`PaneId`], so a program can address its own pane |
//! | `TEMOR_SOCKET` | the caller's control socket path, when there is one; removed otherwise |
//!
//! Variables read by temor and its scripts:
//!
//! | Variable | Meaning |
//! |---|---|
//! | `TEMOR_SHELL_INTEGRATION` | `0` turns shell integration off: no shims are injected, and the scripts do nothing if sourced by hand |
//! | `SHELL` | the shell to use when the passwd lookup has none ([`resolve_shell`]) |
//! | `TERMINFO`, `TERMINFO_DIRS`, `HOME` | where [`find_term`] looks for `xterm-ghostty` |
//! | `ZDOTDIR`, `XDG_DATA_DIRS` | kept and restored by the zsh and fish shims |
//!
//! Variables used only between temor and its own scripts, set by [`plan_integration`] and
//! removed by the scripts before any of the person's code runs: `TEMOR_BASH_LOGIN`,
//! `TEMOR_ZSH_ZDOTDIR` and `TEMOR_FISH_XDG_DATA_DIRS`.
//!
//! # Shell integration
//!
//! The scripts in `shell/` are embedded with `include_str!` and written under the caller's
//! runtime directory, never under HOME:
//! - **bash:** `--rcfile <runtime>/…/bash/temor-bashrc`, a wrapper that sources the person's
//!   `~/.bashrc` (or, for a login start, `/etc/profile` and the first of `~/.bash_profile`,
//!   `~/.bash_login`, `~/.profile`), then `temor.bash`.
//! - **zsh:** `ZDOTDIR` points at a shim directory whose `.zshenv` restores the real `ZDOTDIR`,
//!   sources the real `.zshenv`, then `temor.zsh`; zsh then reads the person's other startup
//!   files from the real `ZDOTDIR` as usual.
//! - **fish:** a `vendor_conf.d` snippet found through `XDG_DATA_DIRS`, which restores the real
//!   `XDG_DATA_DIRS` first.
//!
//! Each script emits OSC 133 `A` (prompt start), `B` (prompt end), `C` (command output starts)
//! and `D;<exit status>` (command finished), and OSC 7 `file://<host><percent-encoded cwd>`.

mod env;
mod error;
mod platform;
mod pty;
mod shell;
mod size;

pub use env::{
    ChildEnv, EnvContract, PaneId, ProgramVersion, SYSTEM_TERMINFO_DIRS, TermName, base_env,
    child_env, find_term, terminfo_dirs,
};
pub use error::PtyError;
pub use pty::{
    ChildExit, Foreground, Interest, ProcRoot, ProcessId, Pty, PtyIo, ReadOutcome, Readiness,
    Ready, SessionLauncher, SignalKind, SpawnSpec, WriteOutcome, find_program, run_session_leader,
};
pub use shell::{
    IntegrationPlan, IntegrationState, Shell, ShellKind, ShellStart, ShimFile, UserId,
    passwd_login_shell, plan_integration, prepare_shell, resolve_shell, write_shims,
};
pub use size::{Cols, GridSize, Px, Rows};

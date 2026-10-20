//! The terminfo source the bootstrap can compile. (Where each integration script goes on the host
//! is `temor_shell::ShellScript::relative_path`, the same layout as the local shims.)

/// The terminfo source for `xterm-ghostty` (`infocmp -x` output), compiled with `tic` only when
/// the host lacks the entry.
pub(super) const TERMINFO_SOURCE: &str = include_str!("../../terminfo/xterm-ghostty.src");

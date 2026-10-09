//! Where each integration script goes on the host, and the terminfo source the bootstrap can
//! compile. The layout matches `temor_shell`'s local shims, so the same scripts run in both.

use temor_shell::ShellScript;

/// The terminfo source for `xterm-ghostty` (`infocmp -x` output), compiled with `tic` only when
/// the host lacks the entry.
pub(super) const TERMINFO_SOURCE: &str = include_str!("../../terminfo/xterm-ghostty.src");

/// The path under the temp directory for an integration script, by its `temor_shell` name, or
/// `None` for a script this bootstrap does not place.
pub(super) fn remote_path(script: &ShellScript) -> Option<&'static str> {
    match script.name {
        "temor-bashrc" => Some("bash/temor-bashrc"),
        "temor.bash" => Some("bash/temor.bash"),
        "zshenv" => Some("zsh/.zshenv"),
        "temor.zsh" => Some("zsh/temor.zsh"),
        "temor.fish" => Some("fish/fish/vendor_conf.d/temor.fish"),
        // Placed now; starting nushell on the host with it is P13 (FOLLOWUPS "temor-ssh").
        "temor.nu" => Some("nu/nushell/vendor/autoload/temor.nu"),
        _ => None,
    }
}

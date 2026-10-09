//! The bootstrap's text: quoting, heredocs and the placement of the integration files. The
//! script's behaviour is tested by running it, in `tests/it`.

use temor_pty::integration_scripts;

use super::BootstrapScript;
use super::files::remote_path;
use super::quote::{heredoc_write, sh_single_quote};
use crate::error::SshError;

#[test]
fn single_quoting_survives_every_metacharacter() {
    assert_eq!(sh_single_quote("a'b"), "'a'\\''b'");
    assert_eq!(
        sh_single_quote("$(id) `x` \"q\"\nline"),
        "'$(id) `x` \"q\"\nline'"
    );
    assert_eq!(sh_single_quote(""), "''");
}

#[test]
fn heredoc_bodies_cannot_end_early() {
    let err = heredoc_write("\"$x\"", "fine\nTEMOR_EOF\nafter").expect_err("conflict");
    assert!(matches!(err, SshError::HeredocConflict(_)));
    assert!(heredoc_write("\"$x\"", "TEMOR_EOF_not\n").is_ok());
}

#[test]
fn every_pty_integration_script_has_a_place() {
    for script in integration_scripts() {
        assert!(
            remote_path(&script).is_some(),
            "no place for {}",
            script.name
        );
    }
}

#[test]
fn built_script_places_every_file_and_no_marks_remain() {
    let script = BootstrapScript::build(&integration_scripts()).expect("build");
    let text = script.as_str();
    assert!(!text.contains("#@INTEGRATION@"));
    assert!(!text.contains("#@TERMINFO@"));
    for target in [
        "\"$temor_tmp/bash/temor-bashrc\"",
        "\"$temor_tmp/bash/temor.bash\"",
        "\"$temor_tmp/zsh/.zshenv\"",
        "\"$temor_tmp/zsh/temor.zsh\"",
        "\"$temor_tmp/fish/fish/vendor_conf.d/temor.fish\"",
        "\"$temor_tmp/xterm-ghostty.src\"",
    ] {
        assert!(
            text.contains(&format!("cat > {target} <<'TEMOR_EOF'")),
            "missing {target}"
        );
    }
    assert!(text.len() < 96 * 1024);
}

#[test]
fn remote_command_is_one_quoted_sh_invocation() {
    let script = BootstrapScript::build(&integration_scripts()).expect("build");
    let remote = super::bootstrap_remote_command(&script);
    assert!(remote.as_str().starts_with("sh -c '"));
    assert!(remote.as_str().ends_with('\''));
}

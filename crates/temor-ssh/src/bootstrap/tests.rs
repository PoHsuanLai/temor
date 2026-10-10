//! The bootstrap's text: heredocs, the placement of the integration files and the transport.
//! The script's behaviour is tested by running it, in `tests/it`.

use temor_shell::integration_scripts;

use super::files::remote_path;
use super::quote::heredoc_write;
use super::{BootstrapOptions, BootstrapScript, TerminfoPolicy};
use super::{MAX_REMOTE_COMMAND_BYTES, wrap_script};
use crate::error::SshError;

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
    assert!(!text.contains("#@OPTIONS@"));
    assert!(text.contains("\ntemor_terminfo=install\n"));
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
}

#[test]
fn remote_command_is_plain_ascii_with_no_quote_or_backslash_inside() {
    let script = BootstrapScript::build(&integration_scripts()).expect("build");
    let remote = super::bootstrap_remote_command(&script).expect("fits");
    let text = remote.as_str();
    assert!(text.is_ascii());
    assert!(text.starts_with("sh -c 's=$(printf %s "));
    assert!(text.ends_with("eval \"$s\"'"), "{text}");
    assert!(text.contains("needs base64 or openssl"), "{text}");
    let inside = &text["sh -c '".len()..text.len() - 1];
    assert!(!inside.contains('\''), "a quote inside the quoted part");
    assert!(!inside.contains('\\'), "a backslash inside the quoted part");
    assert!(text.len() <= MAX_REMOTE_COMMAND_BYTES);
}

#[test]
fn oversize_payloads_are_refused_at_the_argv_level() {
    let big = "x".repeat(MAX_REMOTE_COMMAND_BYTES);
    let err = wrap_script(&big).expect_err("too big");
    assert!(matches!(err, SshError::ScriptTooLarge { .. }), "{err}");
}

#[test]
fn skipping_terminfo_leaves_out_the_source_and_says_so() {
    let options = BootstrapOptions::with_terminfo(TerminfoPolicy::Skip);
    let script = BootstrapScript::build_with(&integration_scripts(), options).expect("build");
    let text = script.as_str();
    assert!(text.contains("\ntemor_terminfo=skip\n"));
    assert!(!text.contains("xterm-ghostty.src\" <<"));
}

#[test]
fn a_start_folder_is_quoted_into_the_options_and_absent_otherwise() {
    let options = BootstrapOptions::default();
    let plain = BootstrapScript::build_in(&integration_scripts(), options, None).expect("build");
    assert!(plain.as_str().contains("\ntemor_start_dir=\n"));
    let dir = crate::files::RemoteFilePath::new("/srv/it's here").expect("path");
    let script =
        BootstrapScript::build_in(&integration_scripts(), options, Some(&dir)).expect("build");
    assert!(
        script
            .as_str()
            .contains("\ntemor_start_dir='/srv/it'\\''s here'\n"),
        "{}",
        script.as_str()
    );
}

#[test]
fn inner_line_is_one_quiet_line_for_bash_and_zsh() {
    use super::InnerLoadLine;
    let line = InnerLoadLine::build(&integration_scripts()).expect("build");
    let text = line.as_str();
    assert!(
        text.starts_with(' '),
        "a leading space keeps it out of history"
    );
    assert_eq!(text.matches('\n').count(), 1);
    assert!(text.ends_with("\"$BASH_VERSION\")\"\n"));
    assert!(text.is_ascii());
    assert!(!text.contains('\\'));
}

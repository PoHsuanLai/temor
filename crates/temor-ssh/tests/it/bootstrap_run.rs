//! The bootstrap run by a real `sh` (here bash, which is what `sh` is on this machine) with a
//! scratch HOME: integration marks appear, dotfiles are untouched, the temp dir is removed,
//! the exit status passes through, and hostile names stay data.

use std::fs;
use std::process::Command;

use temor_shell::integration_scripts;
use temor_ssh::{BootstrapOptions, BootstrapScript, TerminfoPolicy, bootstrap_remote_command};

use crate::harness::{
    Launch, contains, make_bin, require, run, scratch, snapshot, system_path, tool, write,
};

const PRINT_TERM: &str = "echo \"T=$TERM P=$FROM_PROFILE\"\n";

fn dotfiles(home: &std::path::Path) {
    write(&home.join(".bashrc"), "export FROM_BASHRC=yes\n");
    write(&home.join(".bash_profile"), "export FROM_PROFILE=yes\n");
    write(&home.join(".profile"), "export FROM_PROFILE=profile\n");
}

fn bash() -> String {
    tool("bash").expect("bash").display().to_string()
}

#[test]
fn bootstrap_passes_sh_n_and_bash_n() {
    let s = scratch();
    let script = BootstrapScript::build(&integration_scripts()).expect("build");
    let file = s.work.join("bootstrap.sh");
    write(&file, script.as_str());
    for shell in ["sh", "bash"] {
        let program = tool(shell).expect("shell");
        let status = Command::new(program)
            .arg("-n")
            .arg(&file)
            .status()
            .expect("run -n");
        assert!(status.success(), "{shell} -n failed");
    }
    // dash is not installed here; `sh -n` above still checks the POSIX grammar on bash.
}

#[test]
fn marks_appear_dotfiles_stay_and_the_temp_dir_is_removed() {
    let s = scratch();
    dotfiles(&s.home);
    let before = snapshot(&s.home);
    let ran = run(
        &s,
        &system_path(),
        &bash(),
        &format!("{PRINT_TERM}true\nexit\n"),
    );
    assert!(ran.status.success(), "status {:?}", ran.status);
    for mark in [
        "\x1b]133;A",
        "\x1b]133;B",
        "\x1b]133;C",
        "\x1b]133;D;0",
        "\x1b]7;file://",
    ] {
        assert!(contains(&ran.text, mark), "missing {mark:?}");
    }
    assert!(
        contains(&ran.text, "P=yes"),
        "the login profile was not read"
    );
    assert_eq!(snapshot(&s.home), before, "a dotfile changed");
    assert_eq!(
        fs::read_dir(&s.tmp).expect("tmp").count(),
        0,
        "temp dir left behind"
    );
}

#[test]
fn the_exit_status_passes_through_and_cleanup_still_runs() {
    let s = scratch();
    let ran = run(&s, &system_path(), "/nonexistent/shell", "exit 3\n");
    assert_eq!(ran.status.code(), Some(3));
    assert_eq!(fs::read_dir(&s.tmp).expect("tmp").count(), 0);
}

#[test]
fn terminfo_is_compiled_into_the_home_when_the_host_lacks_it() {
    let s = scratch();
    // A failing infocmp makes the host look like it has no Ghostty entry.
    make_bin(
        &s.bin,
        &[
            "bash", "sh", "cat", "mkdir", "mktemp", "rm", "cp", "rmdir", "uname", "hostname",
            "base64", "tic", "printf",
        ],
        &[("infocmp", "#!/bin/sh\nexit 1\n")],
    );
    let path = s.bin.display().to_string();
    let ran = run(&s, &path, &bash(), &format!("{PRINT_TERM}exit\n"));
    assert!(ran.status.success());
    assert!(
        s.home.join(".terminfo/x/xterm-ghostty").is_file(),
        "tic did not write ~/.terminfo"
    );
    assert!(contains(&ran.text, "T=xterm-ghostty"));
}

#[test]
fn a_host_that_did_not_consent_gets_no_terminfo_even_with_tic() {
    let s = scratch();
    make_bin(
        &s.bin,
        &[
            "bash", "sh", "cat", "mkdir", "mktemp", "rm", "cp", "rmdir", "uname", "hostname",
            "base64", "tic", "printf",
        ],
        &[("infocmp", "#!/bin/sh\nexit 1\n")],
    );
    let options = BootstrapOptions::with_terminfo(TerminfoPolicy::Skip);
    let script = BootstrapScript::build_with(&integration_scripts(), options).expect("build");
    let mut launch = Launch::new(&s);
    launch.command = bootstrap_remote_command(&script)
        .expect("fits")
        .as_str()
        .to_owned();
    launch.path = s.bin.display().to_string();
    launch.shell = bash();
    launch.input = format!("{PRINT_TERM}exit\n");
    let ran = launch.run();
    assert!(ran.status.success());
    assert!(contains(&ran.text, "T=xterm-256color"));
    assert!(!s.home.join(".terminfo").exists());
}

#[test]
fn without_tic_the_term_falls_back_and_nothing_is_written() {
    let s = scratch();
    make_bin(
        &s.bin,
        &[
            "bash", "sh", "cat", "mkdir", "mktemp", "rm", "cp", "rmdir", "uname", "hostname",
            "base64", "printf",
        ],
        &[("infocmp", "#!/bin/sh\nexit 1\n")],
    );
    let path = s.bin.display().to_string();
    let ran = run(&s, &path, &bash(), &format!("{PRINT_TERM}exit\n"));
    assert!(ran.status.success());
    assert!(contains(&ran.text, "T=xterm-256color"));
    assert!(!s.home.join(".terminfo").exists());
}

#[test]
fn a_hostile_host_name_stays_data() {
    let s = scratch();
    // The name a host reports, with quotes, a command substitution, a backtick and a newline.
    let hostile = "evil'$(touch PWNED)\n\"x`id`\n";
    make_bin(
        &s.bin,
        &[
            "bash", "sh", "cat", "mkdir", "mktemp", "rm", "cp", "rmdir", "uname", "printf",
        ],
        &[(
            "hostname",
            &format!("#!/bin/sh\ncat <<'FAKE_EOF'\n{hostile}FAKE_EOF\n"),
        )],
    );
    let path = format!("{}:{}", s.bin.display(), system_path());
    let ran = run(&s, &path, &bash(), "exit\n");
    assert!(ran.status.success());
    assert!(
        contains(
            &ran.text,
            "\x1b]7;file://evil%27%24%28touch%20PWNED%29%0A%22x%60id%60/",
        ),
        "the host name was not percent-encoded"
    );
    assert!(
        !s.work.join("PWNED").exists(),
        "the host name ran as a command"
    );
    assert!(!s.home.join("PWNED").exists());
}

#[test]
fn zsh_and_fish_load_their_shims_when_installed() {
    for (name, input) in [
        ("zsh", "print -r -- \"Z=$ZDOTDIR\"\nexit\n"),
        ("fish", "echo F=$XDG_DATA_DIRS\nexit\n"),
    ] {
        let Some(shell) = require(name) else {
            continue;
        };
        let s = scratch();
        let ran = run(&s, &system_path(), &shell.display().to_string(), input);
        assert!(ran.status.success(), "{name} exited {:?}", ran.status);
        if name == "zsh" {
            assert!(
                contains(&ran.text, "\x1b]133;A"),
                "zsh printed no prompt mark"
            );
        } else {
            // fish 4 draws no prompt on piped stdin, with or without temor. Its shim removes
            // temor's XDG_DATA_DIRS entry at startup, so the temp path must be gone here.
            let tmp = s.tmp.display().to_string();
            assert!(contains(&ran.text, "F="), "fish did not run its input");
            assert!(
                !contains(&ran.text, &tmp),
                "fish kept temor's XDG_DATA_DIRS entry"
            );
        }
        assert_eq!(
            fs::read_dir(&s.tmp).expect("tmp").count(),
            0,
            "{name}: temp dir left"
        );
    }
}

#[test]
fn a_host_without_base64_or_openssl_says_so_and_fails() {
    let s = scratch();
    make_bin(&s.bin, &["bash", "sh", "printf", "cat"], &[]);
    let path = s.bin.display().to_string();
    let ran = run(&s, &path, &bash(), &format!("{PRINT_TERM}exit\n"));
    let text = String::from_utf8_lossy(&ran.text).into_owned();
    assert_eq!(ran.status.code(), Some(1), "{text}");
    assert!(text.contains("needs base64 or openssl"), "{text}");
}

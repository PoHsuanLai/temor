//! Exact argv vectors and the rules that keep hostile names out of ssh's option parser.

use std::path::Path;

use super::{
    BindAddress, ControlLayout, ControlPersist, Forward, RemoteCommand, RemoteHost, RuntimeDir,
    Seconds, cancel_forward_argv, check_argv, exit_argv, forward_argv, session_argv,
};
use crate::config::{HostAlias, Port};
use crate::error::SshError;

fn layout() -> ControlLayout {
    let runtime = RuntimeDir::new(Path::new("/run/user/1000/temor")).expect("runtime");
    let persist = ControlPersist::Idle(Seconds::new(600).expect("seconds"));
    ControlLayout::new(runtime, persist).expect("layout")
}

fn alias(text: &str) -> HostAlias {
    HostAlias::new(text).expect("alias")
}

fn forward() -> Forward {
    Forward {
        bind: Some(BindAddress::new("127.0.0.1").expect("bind")),
        local: Port::parse("8080").expect("port"),
        remote_host: RemoteHost::new("localhost").expect("host"),
        remote: Port::parse("3000").expect("port"),
    }
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| (*p).to_owned()).collect()
}

#[test]
fn session_vector_shares_the_master_and_runs_the_bootstrap() {
    let remote = RemoteCommand::new("sh -c 'echo hi'".to_owned());
    let argv = session_argv(&alias("prod"), &layout(), &remote);
    assert_eq!(
        argv,
        strings(&[
            "-t",
            "-o",
            "ControlMaster=auto",
            "-o",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "-o",
            "ControlPersist=600",
            "--",
            "prod",
            "sh -c 'echo hi'",
        ])
    );
}

#[test]
fn control_commands_use_only_the_control_path() {
    let target = alias("prod");
    assert_eq!(
        check_argv(&target, &layout()),
        strings(&[
            "-O",
            "check",
            "-o",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "--",
            "prod"
        ])
    );
    assert_eq!(
        exit_argv(&target, &layout()),
        strings(&[
            "-O",
            "exit",
            "-o",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "--",
            "prod"
        ])
    );
}

#[test]
fn forwards_add_and_cancel_on_the_running_master() {
    let target = alias("user@prod:2222");
    assert_eq!(
        forward_argv(&target, &layout(), &forward()),
        strings(&[
            "-O",
            "forward",
            "-o",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "-L",
            "127.0.0.1:8080:localhost:3000",
            "--",
            "user@prod:2222",
        ])
    );
    let cancel = cancel_forward_argv(&target, &layout(), &forward());
    assert_eq!(
        cancel,
        strings(&[
            "-O",
            "cancel",
            "-o",
            "ControlPath=/run/user/1000/temor/ssh/%C",
            "-L",
            "127.0.0.1:8080:localhost:3000",
            "--",
            "user@prod:2222",
        ])
    );
}

#[test]
fn forward_without_bind_and_ipv6_hosts_are_bracketed() {
    let plain = Forward {
        bind: None,
        local: Port::parse("8080").expect("port"),
        remote_host: RemoteHost::new("::1").expect("host"),
        remote: Port::parse("3000").expect("port"),
    };
    assert_eq!(plain.spec().as_str(), "8080:[::1]:3000");
    let bound = Forward {
        bind: Some(BindAddress::new("::1").expect("bind")),
        ..plain
    };
    assert_eq!(bound.spec().as_str(), "[::1]:8080:[::1]:3000");
}

#[test]
fn persist_forever_is_yes() {
    let runtime = RuntimeDir::new(Path::new("/run/temor")).expect("runtime");
    let layout = ControlLayout::new(runtime, ControlPersist::Forever).expect("layout");
    assert_eq!(layout.control_persist_option(), "ControlPersist=yes");
    assert!(Seconds::new(0).is_none());
}

#[test]
fn hostile_destinations_never_reach_argv() {
    for hostile in [
        "-oProxyCommand=evil",
        "$(touch x)",
        "prod;rm -rf ~",
        "a'b",
        "a\"b",
        "a\nb",
        "a b",
        "`id`",
        "",
    ] {
        assert!(
            matches!(HostAlias::new(hostile), Err(SshError::InvalidAlias(_))),
            "accepted {hostile:?}"
        );
    }
}

#[test]
fn destinations_follow_the_option_terminator() {
    let argv = check_argv(&alias("prod"), &layout());
    let dashes = argv.iter().position(|a| a == "--").expect("terminator");
    assert_eq!(argv[dashes + 1], "prod");
    assert_eq!(dashes + 2, argv.len());
}

#[test]
fn runtime_dirs_must_be_plain_absolute_paths() {
    for bad in [
        "relative/dir",
        "/run/a/../b",
        "/run/a b",
        "/run/a'b",
        "/run/a$b",
    ] {
        assert!(RuntimeDir::new(Path::new(bad)).is_err(), "accepted {bad}");
    }
    assert!(RuntimeDir::new(Path::new("/run/user/1000/temor")).is_ok());
}

#[test]
fn control_socket_path_must_fit_a_unix_socket() {
    let long = format!("/{}", "d".repeat(80));
    let runtime = RuntimeDir::new(Path::new(&long)).expect("runtime");
    let err = ControlLayout::new(runtime, ControlPersist::Forever).expect_err("too long");
    assert!(matches!(err, SshError::ControlPathTooLong { .. }), "{err}");
}

#[test]
fn forward_hosts_reject_spaces_and_shell_text() {
    assert!(RemoteHost::new("local host").is_err());
    assert!(RemoteHost::new("$(id)").is_err());
    assert!(BindAddress::new("").is_err());
}

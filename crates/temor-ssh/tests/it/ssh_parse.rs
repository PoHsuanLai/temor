//! The argv the crate builds is parsed by the system `ssh` with `-G`, which prints the resolved
//! configuration and never connects. The config is a scratch file, not `~/.ssh/config`.

use std::path::Path;
use std::process::Command;

use temor_ssh::{
    ControlLayout, ControlPersist, HostAlias, RuntimeDir, Seconds, check_argv, session_argv,
};

use crate::harness::{remote, scratch, tool, write};

#[test]
fn ssh_accepts_the_session_and_control_vectors() {
    let Some(ssh) = tool("ssh") else {
        eprintln!("skipping: no ssh on this machine");
        return;
    };
    let s = scratch();
    let config = s.work.join("ssh_config");
    write(&config, "Host prod\n  HostName 192.0.2.10\n");
    let layout = ControlLayout::new(
        RuntimeDir::new(Path::new("/run/user/1000/temor")).expect("runtime"),
        ControlPersist::Idle(Seconds::new(600).expect("seconds")),
    )
    .expect("layout");
    let alias = HostAlias::new("prod").expect("alias");

    let mut session = vec![
        "-G".to_owned(),
        "-F".to_owned(),
        config.display().to_string(),
    ];
    session.extend(session_argv(&alias, &layout, &remote()));
    let out = Command::new(&ssh)
        .args(&session)
        .env("HOME", &s.home)
        .output()
        .expect("ssh -G");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "ssh -G rejected the session: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("hostname 192.0.2.10"), "{text}");
    assert!(text.contains("controlmaster auto"), "{text}");
    assert!(text.contains("controlpersist 600"), "{text}");
    // ssh expands %C into a 40-digit hex hash, so check the directory and that hash's shape.
    let hash = text
        .lines()
        .find_map(|l| l.strip_prefix("controlpath /run/user/1000/temor/ssh/"))
        .unwrap_or_default();
    assert!(
        hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()),
        "{text}"
    );

    let mut check = vec![
        "-G".to_owned(),
        "-F".to_owned(),
        config.display().to_string(),
    ];
    check.extend(check_argv(&alias, &layout));
    let out = Command::new(&ssh)
        .args(&check)
        .env("HOME", &s.home)
        .output()
        .expect("ssh -G");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

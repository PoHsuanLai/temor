//! The questions before a first connection, how `ssh` starts, sharing the connection between
//! panes, and the browser check.

use temor_client::model::host::consent::BootstrapAction;
use temor_client::model::remote::Grant;
use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::text::Cwd;
use temor_client::model::workspace::WorkspaceIn;

use super::support::Rig;
use crate::hub::ssh_view::{CheckRow, Stage};

/// Longer than the 80-column screen, so on screen it wraps; the link is read from the bytes.
const CHECK_URL: &str =
    "https://login.tailscale.com/a/abc123?token=0123456789012345678901234567890123456789";
const CHECK_LINE: &str = "# To authenticate, visit: https://login.tailscale.com/a/abc123?token=0123456789012345678901234567890123456789\n";

/// The arguments the stand-in `ssh` was started with for session `n`.
fn session(rig: &Rig, n: u32) -> Vec<String> {
    let text = rig.got(&format!("session-{n}")).unwrap_or_default();
    text.lines().map(str::to_owned).collect()
}

#[test]
fn each_question_is_asked_alone_and_the_answers_decide_how_ssh_starts() {
    // Name, the answer to the setup, the answer to the terminal file (asked only after a yes),
    // whether the session carries the setup, and the file policy the setup was built with.
    let table = [
        (
            "both yes",
            Grant::Allowed,
            Some(Grant::Allowed),
            true,
            "install",
        ),
        (
            "no file",
            Grant::Allowed,
            Some(Grant::Refused),
            true,
            "skip",
        ),
        ("plain", Grant::Refused, None, false, ""),
    ];
    for (name, integrate, terminfo, bootstrap, policy) in table {
        let Some(mut rig) = Rig::new(&format!("asks-{}", name.replace(' ', "-"))) else {
            return;
        };
        rig.hub.open_host_named("fake");
        let pane = rig.front();
        assert_eq!(
            rig.stage(pane),
            Stage::Asking(BootstrapAction::Integrate),
            "{name}"
        );
        rig.hub.answer_host(pane, integrate);
        match terminfo {
            Some(grant) => {
                assert_eq!(
                    rig.stage(pane),
                    Stage::Asking(BootstrapAction::Terminfo),
                    "{name}"
                );
                rig.hub.answer_host(pane, grant);
            }
            None => assert_eq!(
                rig.stage(pane),
                Stage::Connecting,
                "{name}: no second question"
            ),
        }
        let dir = rig.dir.clone();
        rig.pump("the session to start", |_| dir.join("session-1").exists());

        let argv = session(&rig, 1);
        assert_eq!(&argv[..2], ["-t", "-o"], "{name}");
        assert!(argv.contains(&"ControlMaster=auto".to_owned()), "{name}");
        let tail = &argv[argv.len() - 2..];
        match bootstrap {
            true => {
                assert_eq!(
                    (&argv[argv.len() - 3], &tail[0]),
                    (&"--".to_owned(), &"fake".to_owned())
                );
                assert!(
                    argv.last()
                        .is_some_and(|c| c.starts_with("sh -c 's=$(printf %s ")),
                    "{name}"
                );
                // The fake writes the script as it decodes it: wait for its whole setup line,
                // not only for the file.
                let line = format!("\ntemor_terminfo={policy}\n");
                rig.pump("the setup to be decoded", |_| {
                    std::fs::read_to_string(dir.join("session-1.script"))
                        .is_ok_and(|script| script.contains(&line))
                });
                let script = rig.got("session-1.script").unwrap_or_default();
                assert!(
                    script.contains(&format!("\ntemor_terminfo={policy}\n")),
                    "{name}"
                );
            }
            false => assert_eq!(tail, ["--", "fake"], "{name}: plain ssh has no command"),
        }
        // What was answered is kept for the machine, so the session file can hold it.
        let book = rig.hub.ssh.hosts_file().book;
        assert_eq!(book.records().len(), 1, "{name}");
        assert_eq!(book.records()[0].aliases, ["fake"], "{name}");
        assert_eq!(book.records()[0].grants.integrate, integrate, "{name}");
    }
}

#[test]
fn a_host_that_was_answered_is_not_asked_again() {
    let Some(mut rig) = Rig::new("asks-once") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let first = rig.front();
    rig.hub.answer_host(first, Grant::Allowed);
    rig.hub.answer_host(first, Grant::Allowed);
    rig.hub.open_host_named("fake");
    let second = rig.front();
    assert_ne!(first, second);
    assert_eq!(rig.stage(second), Stage::Connecting);
}

#[test]
fn a_split_or_new_tab_of_a_host_pane_joins_its_connection_in_its_folder() {
    let Some(mut rig) = Rig::new("share") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let first = rig.front();
    rig.hub.answer_host(first, Grant::Allowed);
    rig.hub.answer_host(first, Grant::Allowed);
    let dir = rig.dir.clone();
    rig.pump("the first session", |_| dir.join("session-1").exists());
    // The host answers `-O check` from now on: the connection is up.
    rig.put("up", "");
    rig.pump("the connection to be up", |hub| {
        hub.host_view(first).is_some_and(|v| v.stage == Stage::Up)
    });
    rig.hub.act(WorkspaceIn::Cwd {
        pane: first,
        cwd: Cwd("/srv/it's here".to_owned()),
    });

    // A split, then a new tab: each is a session on the same host, started in that folder, and
    // neither asks anything.
    for (n, input) in [
        (
            2,
            WorkspaceIn::Split {
                towards: temor_client::model::split::Direction::Right,
            },
        ),
        (3, WorkspaceIn::NewTab { cwd: None }),
    ] {
        rig.hub.act(input);
        let pane = rig.front();
        assert_eq!(rig.hub.host_of(pane), Some("fake"), "session {n}");
        let name = format!("session-{n}.script");
        // The fake host writes the script as it reads it: wait for its folder line, not the file.
        rig.pump("the joined session", |_| {
            std::fs::read_to_string(dir.join(&name)).is_ok_and(|s| s.contains("temor_start_dir="))
        });
        let script = rig.got(&name).unwrap_or_default();
        assert!(
            script.contains("\ntemor_start_dir='/srv/it'\\''s here'\n"),
            "session {n}:\n{script}"
        );
        assert_eq!(rig.stage(pane), Stage::Up, "session {n}");
    }
    let title = rig
        .hub
        .sidebar()
        .loose
        .iter()
        .map(|row| row.title.clone())
        .collect::<Vec<_>>();
    assert!(
        title.iter().any(|t| t == "fake:/srv/it's here"),
        "{title:?}"
    );
}

#[test]
fn the_browser_check_is_shown_and_opens_only_on_a_click_and_goes_when_the_shell_starts() {
    let Some(mut rig) = Rig::new("check") else {
        return;
    };
    rig.put("banner", CHECK_LINE);
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.pump("the check row", |hub| {
        hub.host_view(pane)
            .is_some_and(|v| v.check == CheckRow::Waiting)
    });
    assert!(
        rig.links.0.borrow().is_empty(),
        "nothing opened before a click"
    );

    rig.hub.open_check(pane);
    assert_eq!(*rig.links.0.borrow(), [CHECK_URL]);

    // The connection comes up (the person confirmed): the row goes.
    rig.put("up", "");
    rig.pump("the connection", |hub| {
        hub.host_view(pane)
            .is_some_and(|v| v.check == CheckRow::Nothing && v.stage == Stage::Up)
    });
}

#[test]
fn the_connection_is_up_when_its_socket_appears_without_asking_ssh() {
    let Some(mut rig) = Rig::new("socket") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.pump("the session to start", |hub| {
        hub.host_view(pane)
            .is_some_and(|v| v.stage == Stage::Connecting)
            && hub
                .ssh
                .baselines
                .keys()
                .any(|alias| alias.as_str() == "fake")
    });

    // The stand-in's `-O check` says no (there is no `up` file): only the socket can say it.
    let socket_dir = rig
        .hub
        .ssh
        .env
        .layout
        .as_ref()
        .expect("a layout")
        .socket_dir();
    std::fs::write(socket_dir.join("c".repeat(40)), "").expect("the master's socket");
    rig.pump("the connection", |hub| {
        hub.host_view(pane).is_some_and(|v| v.stage == Stage::Up)
    });
}

#[test]
fn a_plain_aliass_fingerprint_is_looked_up_by_the_name_and_port_ssh_resolves() {
    let Some(mut rig) = Rig::new("identify") else {
        return;
    };
    rig.put(
        "fp",
        "256 SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s resolved.example.com (ED25519)\n",
    );
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    // Up only once the first session is making the connection: an `up` before that is a
    // connection already there, which is joined, not identified.
    let dir = rig.dir.clone();
    rig.pump("the first session", |_| dir.join("session-1").exists());
    rig.put("up", "");
    let alias = temor_client::ssh::HostAlias::new("fake").expect("alias");
    rig.pump("the host to be known by its key", |hub| {
        hub.ssh
            .lines
            .get(&alias)
            .is_some_and(|line| line.key.is_some())
    });
    // Asked for by the HostName and port of `ssh -G`, in the form ssh records a port in, and
    // not by the alias.
    assert_eq!(
        rig.got("keygen-args").as_deref(),
        Some("-F\n[resolved.example.com]:2200\n-l\n")
    );
}

#[test]
fn a_plain_session_sends_nothing_so_a_split_of_it_starts_in_the_hosts_home_folder() {
    let Some(mut rig) = Rig::new("plain-split") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let first = rig.front();
    rig.hub.answer_host(first, Grant::Refused);
    let dir = rig.dir.clone();
    rig.pump("the first session", |_| dir.join("session-1").exists());
    rig.put("up", "");
    rig.pump("the connection to be up", |hub| {
        hub.host_view(first).is_some_and(|v| v.stage == Stage::Up)
    });
    rig.hub.act(WorkspaceIn::Cwd {
        pane: first,
        cwd: Cwd("/srv/it".to_owned()),
    });
    rig.hub.act(WorkspaceIn::Split {
        towards: temor_client::model::split::Direction::Right,
    });
    assert_eq!(rig.hub.host_of(rig.front()), Some("fake"));
    rig.pump("the joined session", |_| dir.join("session-2").exists());
    let args = session(&rig, 2);
    assert_eq!(
        args[args.len() - 2..],
        ["--", "fake"],
        "no remote command: {args:?}"
    );
}

#[test]
fn a_restored_host_tab_connects_when_it_is_first_in_front_not_at_launch() {
    use temor_client::model::host::consent::{BootstrapGrants, ConsentBook, HostRecord};
    use temor_client::model::host::store::PaneHost;

    let Some(mut rig) = Rig::new("restored") else {
        return;
    };
    rig.hub.ssh.book = ConsentBook::from_records(vec![HostRecord {
        aliases: vec!["fake".to_owned()],
        grants: BootstrapGrants {
            integrate: Grant::Allowed,
            terminfo: Grant::Refused,
            ..BootstrapGrants::default()
        },
        ..HostRecord::default()
    }]);
    // The last session had a host tab; it starts hidden behind a plain tab.
    rig.hub.act(WorkspaceIn::NewTab { cwd: None });
    let plain = rig.front();
    let saved = temor_client::model::vocab::ids::PaneId(rig.hub.book.ws.next_pane);
    rig.hub.ssh.restored = vec![PaneHost {
        pane: saved,
        alias: "fake".to_owned(),
        session: None,
    }];
    rig.hub.act(WorkspaceIn::NewTab { cwd: None });
    assert_eq!(rig.hub.host_of(saved), Some("fake"));
    rig.hub.act(WorkspaceIn::Focus(plain));
    assert_eq!(rig.front(), plain);

    let dir = rig.dir.clone();
    let start = std::time::Instant::now();
    rig.pump("some ticks to pass", |_| {
        start.elapsed() > std::time::Duration::from_millis(900)
    });
    assert!(
        !dir.join("session-1").exists() && rig.got("connect-ok").is_none(),
        "nothing connects behind the front tab"
    );

    rig.hub.act(WorkspaceIn::Focus(saved));
    rig.pump("the session once shown", |_| dir.join("session-1").exists());
}

#[test]
fn a_new_host_pane_is_remote_from_its_first_frame() {
    let Some(mut rig) = Rig::new("remote-site") else {
        return;
    };
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    rig.hub.answer_host(pane, Grant::Allowed);
    rig.hub.answer_host(pane, Grant::Allowed);
    let dir = rig.dir.clone();
    rig.pump("the session to start", |_| dir.join("session-1").exists());
    // No prompt or folder report has come, so no read of the pane's facts has been asked for.
    let slot = rig.hub.slots.get(&pane).expect("the pane's slot");
    let site = slot.borrow().session.as_ref().map(|s| s.site());
    assert_eq!(site, Some(PaneSite::Remote));
}

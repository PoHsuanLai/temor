//! The session file and its save pacing, in scratch folders. Nothing touches the real state dir.

use std::path::Path;
use std::time::{Duration, Instant};

use temor_client::keep::older::{
    Startup, load as load_session, load_hosts, resolve_state_dir, save as save_session,
    save_with_hosts,
};
use temor_client::keep::{Pace, SavePace};
use temor_client::model::host::consent::{ConsentBook, HostRecord};
use temor_client::model::host::store::{HostsFile, PaneHost};
use temor_client::model::split::Direction;
use temor_client::model::workspace::restore::FreshWhy;
use temor_client::model::workspace::{
    ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step,
};
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::Cwd;

fn built() -> Workspace {
    let ws = Workspace::new(ClosedState::new(KeepClosed::DEFAULT));
    let steps = [
        WorkspaceIn::NewTab {
            cwd: Some(Cwd("/home/a".into())),
        },
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
    ];
    steps
        .into_iter()
        .fold(ws, |ws, step| workspace_step(ws, step).0)
}

#[test]
fn the_state_dir_follows_xdg_then_home_and_ignores_relative_values() {
    #[rustfmt::skip]
    let cases: &[(Option<&str>, Option<&str>, Option<&str>)] = &[
        (Some("/xdg/state"), Some("/home/someone"), Some("/xdg/state/temor")),
        (Some("relative/state"), Some("/home/someone"), Some("/home/someone/.local/state/temor")),
        (None, Some("/home/someone"), Some("/home/someone/.local/state/temor")),
        (None, None, None),
    ];
    for (xdg, home, want) in cases {
        let got = resolve_state_dir(xdg.map(Path::new), home.map(Path::new));
        assert_eq!(
            got.as_deref(),
            want.map(Path::new),
            "xdg {xdg:?} home {home:?}"
        );
    }
}

#[test]
fn a_saved_session_loads_back_and_the_files_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("temor");
    let ws = built();
    save_session(&state, &ws).unwrap();
    let Startup::Restored(back) = load_session(&state, KeepClosed::DEFAULT).unwrap() else {
        panic!("expected the saved session");
    };
    assert_eq!(back.tabs, ws.tabs);
    assert_eq!(back.cwds, ws.cwds);
    assert!(
        !state.join("session.json.tmp").exists(),
        "the temporary file is renamed away"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = std::fs::metadata(state.join("session.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let folder = std::fs::metadata(&state).unwrap().permissions().mode() & 0o777;
        assert_eq!(file, 0o600);
        assert_eq!(folder, 0o700);
    }
}

#[test]
fn hosts_are_kept_beside_the_tabs_without_disturbing_them() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("temor");
    let ws = built();
    let hosts = HostsFile {
        book: ConsentBook::from_records(vec![HostRecord {
            aliases: vec!["prod".to_owned()],
            ..HostRecord::default()
        }]),
        panes: vec![PaneHost {
            pane: PaneId(1),
            alias: "prod".to_owned(),
            session: None,
        }],
    };
    save_with_hosts(&state, &ws, &hosts).unwrap();
    assert_eq!(load_hosts(&state), hosts);
    let Startup::Restored(back) = load_session(&state, KeepClosed::DEFAULT).unwrap() else {
        panic!("expected the saved session");
    };
    assert_eq!(back.tabs, ws.tabs);
}

#[test]
fn no_file_means_no_session_and_a_damaged_one_is_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        load_session(dir.path(), KeepClosed::DEFAULT).unwrap(),
        Startup::NoFile
    );
    std::fs::write(dir.path().join("session.json"), "{ broken").unwrap();
    assert_eq!(
        load_session(dir.path(), KeepClosed::DEFAULT).unwrap(),
        Startup::Fresh(FreshWhy::Damaged)
    );
    assert!(dir.path().join("session.json.bad").exists());
    assert!(!dir.path().join("session.json").exists());
    // Bytes that are not text are damaged too, and are set aside rather than overwritten.
    std::fs::write(dir.path().join("session.json"), [0xff, 0xfe, 0x00]).unwrap();
    assert_eq!(
        load_session(dir.path(), KeepClosed::DEFAULT).unwrap(),
        Startup::Fresh(FreshWhy::Damaged)
    );
    assert_eq!(
        std::fs::read(dir.path().join("session.json.bad")).unwrap(),
        [0xff, 0xfe, 0x00]
    );
}

#[test]
fn a_newer_file_is_left_alone_even_when_saving() {
    let dir = tempfile::tempdir().unwrap();
    let text = r#"{"version": 7, "tabs": []}"#;
    std::fs::write(dir.path().join("session.json"), text).unwrap();
    assert_eq!(
        load_session(dir.path(), KeepClosed::DEFAULT).unwrap(),
        Startup::Fresh(FreshWhy::NewerVersion(7))
    );
    save_session(dir.path(), &built()).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("session.json")).unwrap(),
        text
    );
}

#[test]
fn saves_are_paced_to_one_per_interval_and_owed_at_quit() {
    let t0 = Instant::now();
    let s = Duration::from_secs;
    let mut pace = SavePace::new(s(1));
    assert_eq!(pace.poll(t0), Pace::Idle);
    pace.changed();
    assert_eq!(
        pace.poll(t0),
        Pace::WriteNow,
        "the first change writes at once"
    );
    pace.wrote(t0);
    pace.changed();
    assert_eq!(
        pace.poll(t0 + Duration::from_millis(400)),
        Pace::WaitFor(Duration::from_millis(600))
    );
    assert_eq!(pace.poll(t0 + s(1)), Pace::WriteNow);
    pace.wrote(t0 + s(1));
    assert!(!pace.owed());
    pace.changed();
    assert!(pace.owed(), "a change not yet written is owed at quit");
}

#[test]
fn checkpoint_files_are_private_read_once_and_hold_no_secret_value() {
    use temor_client::keep::checkpoint::{take, write};
    use temor_client::model::session::{Checkpoint, ShellIn, ShellState};

    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("temor");
    let report = temor_shell::report::Report {
        key: String::new(),
        env: vec![
            ("EDITOR".into(), "vim".into()),
            ("GITHUB_TOKEN".into(), "ghp_never_on_disk".into()),
        ],
        aliases: vec![],
    };
    let mut shell = ShellState::default();
    shell.apply(ShellIn::Reported(report));
    let kept = Checkpoint::of(shell.replay(), std::iter::empty());
    write(&state, PaneId(1), &kept).unwrap();
    let file = state.join("checkpoints/1.json");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("EDITOR") && text.contains("GITHUB_TOKEN"));
    assert!(
        !text.contains("ghp_never_on_disk"),
        "a secret's value is never written"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&state.join("checkpoints")), 0o700);
    }
    assert_eq!(take(&state, PaneId(1)), Some(kept));
    assert!(!file.exists(), "reading takes it");
}

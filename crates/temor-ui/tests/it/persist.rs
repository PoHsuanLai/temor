//! The session file and its save pacing, in scratch folders. Nothing touches the real state dir.

use std::path::Path;
use std::time::{Duration, Instant};

use temor_core::split::Direction;
use temor_core::vocab::text::Cwd;
use temor_core::workspace::restore::FreshWhy;
use temor_core::workspace::{ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step};
use temor_ui::edge::{Pace, SavePace, Startup, load_session, resolve_state_dir, save_session};

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

//! Spaces in the window, in ds-harness (event dispatch only, never OS input), and the Spaces
//! file in scratch folders. Nothing here touches the real state folder or the display.

use std::path::Path;
use std::time::Duration;

use ds::components::app::spaces::{SpaceId, Spaces};
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_client::keep::older::{
    SpacesStartup, load_spaces, save_for as save_session_for, save_spaces,
};
use temor_client::model::space::{Place, Profile, Project, Route};
use temor_client::model::split::Direction;
use temor_client::model::workspace::{
    ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step,
};
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::{Cwd, EnvKey, EnvValue, ProjectRoot, RelPath, RoutePattern};
use temor_ui::view::probe::Probe;

use super::drive::{chord, rows, type_line, wait_until};
use super::support::scratch;
use super::window::harness_in;

fn project(root: Option<&Path>, env: &[(&str, &str)], routes: &[&str]) -> Project {
    Project {
        root: root.map(|r| ProjectRoot(r.display().to_string())),
        profile: Profile {
            env: env
                .iter()
                .map(|(k, v)| (EnvKey::new(*k).expect("a name"), EnvValue((*v).to_owned())))
                .collect(),
            dotenv: root.map(|_| RelPath(".env".to_owned())),
            ..Profile::default()
        },
        routes: routes
            .iter()
            .map(|p| Route {
                pattern: RoutePattern((*p).to_owned()),
            })
            .collect(),
    }
}

#[test]
fn the_spaces_file_round_trips_privately_and_a_damaged_one_is_set_aside() {
    let dir = tempfile::tempdir().expect("scratch");
    let state = dir.path().join("temor");
    assert_eq!(load_spaces(&state).expect("read"), SpacesStartup::NoFile);

    let mut spaces = Spaces::first_run(
        [
            project(Some(Path::new("/srv/work")), &[("A", "1")], &["ssh prod-*"]),
            Project::default(),
        ],
        Project::default,
    );
    spaces.edit_recall(SpaceId(1), |place: &mut Place| {
        place.focused = Some(TabId(3))
    });
    spaces.rename(SpaceId(1), "Ops");
    save_spaces(&state, &spaces).expect("saved");
    let SpacesStartup::Loaded(back) = load_spaces(&state).expect("read") else {
        panic!("expected the saved Spaces");
    };
    assert_eq!(back, spaces);
    assert!(
        !state.join("spaces.json.tmp").exists(),
        "the temporary file is renamed away"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).expect("exists").permissions().mode() & 0o777;
        assert_eq!(mode(&state.join("spaces.json")), 0o600);
        assert_eq!(mode(&state), 0o700);
    }

    // A damaged file is evidence: it is set aside, and the next save writes a clean one.
    let file = state.join("spaces.json");
    #[rustfmt::skip]
    let damaged: &[(&str, &[u8])] = &[
        ("not JSON", b"{ broken"),
        ("not text", &[0xff, 0xfe, 0x00]),
        ("no Space in it", br#"{"spaces": []}"#),
    ];
    for (name, bytes) in damaged {
        std::fs::write(&file, bytes).expect("written");
        assert_eq!(
            load_spaces(&state).expect("read"),
            SpacesStartup::Damaged,
            "{name}"
        );
        assert_eq!(
            std::fs::read(state.join("spaces.json.bad")).expect("set aside"),
            *bytes,
            "{name}"
        );
        assert!(!file.exists(), "{name}");
    }

    // A Space with a key that does not parse still loads, with that key's default.
    let odd = r#"{"spaces":[{"id":4,"name":"Ops","root":42}],"current":0}"#;
    std::fs::write(&file, odd).expect("written");
    let SpacesStartup::Loaded(odd) = load_spaces(&state).expect("read") else {
        panic!("expected the Space");
    };
    assert_eq!(odd.current().name, "Ops");
    assert_eq!(odd.current().payload.root, None);
}

/// Gives the terminal the keyboard, as a click in it would.
fn focus_terminal(h: &mut Harness) {
    h.send(Input::click(Point {
        x: Px(500.0),
        y: Px(250.0),
    }));
    h.advance(Duration::from_millis(200));
}

fn go_to(h: &mut Harness, name: &str) {
    let dot = h
        .centre(&format!("button[aria-label=\"{name} Space\"]"))
        .unwrap_or_else(|| panic!("no dot for {name}:\n{}", h.html()));
    h.send(Input::click(dot));
    h.advance(Duration::from_millis(400));
}

/// Two Spaces: Home (no folder) and Work (a folder, a variable, a dotenv file and two saved
/// tabs). Switching shows each Space's tabs and look, and a new tab in Work starts in its
/// folder with its variables.
#[test]
fn switching_spaces_shows_their_tabs_and_a_new_tab_gets_the_spaces_folder_and_variables() {
    if !super::support::window_tools_present() {
        return;
    }
    let state = scratch("spaces-state").join("state");
    let work = scratch("spaces-state").join("work");
    std::fs::create_dir_all(&work).expect("work folder");
    std::fs::write(work.join(".env"), "FROM_DOTENV=dot-ok\n").expect("dotenv");
    let mut spaces = Spaces::first_run(
        [
            Project::default(),
            project(Some(&work), &[("SPACE_PROBE", "work-ok")], &[]),
        ],
        Project::default,
    );
    spaces.rename(SpaceId(0), "Home");
    spaces.rename(SpaceId(1), "Work");
    save_spaces(&state, &spaces).expect("spaces saved");
    let here = Cwd(work.display().to_string());
    let saved = [
        WorkspaceIn::NewTab {
            cwd: Some(here.clone()),
        },
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
        WorkspaceIn::NewTab { cwd: Some(here) },
    ]
    .into_iter()
    .fold(
        Workspace::new(ClosedState::new(KeepClosed::DEFAULT)),
        |ws, step| workspace_step(ws, step).0,
    );
    save_session_for(&state, SpaceId(1), &saved).expect("work session saved");

    let probe = Probe::new();
    let mut h = harness_in("spaces-window", &probe, Some(state));
    wait_until(&mut h, &probe, "the first prompt", |s| s.contains('$'));
    assert_eq!(
        h.text_of(".ds-space-head").unwrap_or_default().trim(),
        "Home"
    );
    assert_eq!(rows(&h), 1, "Home has its own tab:\n{}", h.html());
    let home_frame = h.attr(".ds", "style").unwrap_or_default();

    focus_terminal(&mut h);
    type_line(&mut h, "echo =$SPACE_PROBE=");
    wait_until(&mut h, &probe, "Home has no Work variable", |s| {
        s.lines().any(|l| l.trim_end() == "==")
    });

    go_to(&mut h, "Work");
    assert_eq!(
        h.text_of(".ds-space-head").unwrap_or_default().trim(),
        "Work"
    );
    assert_eq!(rows(&h), 2, "Work's two saved tabs:\n{}", h.html());
    assert_ne!(
        h.attr(".ds", "style").unwrap_or_default(),
        home_frame,
        "the frame follows the Space"
    );

    // A new tab in Work (⌘N; ⌘T is the bar): its folder, its variable and its dotenv file.
    chord(&mut h, 'n');
    assert_eq!(rows(&h), 3, "a new tab in Work:\n{}", h.html());
    focus_terminal(&mut h);
    type_line(&mut h, "echo =$SPACE_PROBE=$FROM_DOTENV=; basename $PWD");
    wait_until(&mut h, &probe, "Work's variables and folder", |s| {
        let has = |want: &str| s.lines().any(|l| l.trim_end() == want);
        has("=work-ok=dot-ok=") && has("work")
    });

    go_to(&mut h, "Home");
    assert_eq!(rows(&h), 1, "back in Home:\n{}", h.html());
    go_to(&mut h, "Work");
    assert_eq!(rows(&h), 3, "Work kept its tabs:\n{}", h.html());
}

/// A Spaces file that cannot be read is set aside, the window opens with a first Space, and the
/// person is told.
#[test]
fn a_damaged_spaces_file_is_set_aside_and_the_window_starts_with_one_space() {
    if !super::support::window_tools_present() {
        return;
    }
    let state = scratch("spaces-damaged-state").join("state");
    std::fs::create_dir_all(&state).expect("state");
    std::fs::write(state.join("spaces.json"), "{ broken").expect("damaged file");
    let probe = Probe::new();
    let mut h = harness_in("spaces-damaged", &probe, Some(state.clone()));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert!(
        h.html().contains("Your Spaces file was damaged"),
        "{}",
        h.html()
    );
    assert!(state.join("spaces.json.bad").exists());
    assert_eq!(rows(&h), 1);
}

/// The ⌘T bar lists the Spaces: typing a Space's name and Enter switches to it, with no mouse.
#[test]
fn the_bar_switches_spaces_from_the_keyboard() {
    if !super::support::window_tools_present() {
        return;
    }
    let state = scratch("spaces-bar-state").join("state");
    let mut spaces = Spaces::first_run([Project::default(), Project::default()], Project::default);
    spaces.rename(SpaceId(0), "Home");
    spaces.rename(SpaceId(1), "Elsewhere");
    save_spaces(&state, &spaces).expect("spaces saved");
    let probe = Probe::new();
    let mut h = harness_in("spaces-bar", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(
        h.text_of(".ds-space-head").unwrap_or_default().trim(),
        "Home"
    );

    focus_terminal(&mut h);
    chord(&mut h, 't');
    type_line(&mut h, "Elsewhere");
    h.advance(Duration::from_millis(400));
    assert_eq!(
        h.text_of(".ds-space-head").unwrap_or_default().trim(),
        "Elsewhere",
        "the Space row switched:\n{}",
        h.html()
    );
}

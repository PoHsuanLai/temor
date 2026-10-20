//! The window in ds-harness: a real Blitz document with the app's root, driven by the events a
//! window would deliver (never OS input). The pane's text is read from the probe the window
//! publishes after each repaint, since its pixels are a texture.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::SystemPrefs;
use ds::prelude::{Point, Px};
use ds_harness::Input;
use ds_harness::{Harness, HarnessConfig, Viewport, WindowHosting};
use temor_desktop::RunEnds;
use temor_pane::settings::ShellChoice;
use temor_pane::settings::TerminalSettings;
use temor_ui::app::desktop::Desktop;
use temor_ui::edge::Ambient;
use temor_ui::hub::Run;
use temor_ui::view::App;
use temor_ui::view::probe::Probe;
use temor_ui::{AppearanceSource, Launch, LauncherChoice, TakeOnce};

use super::support::scratch;

pub(super) fn harness(name: &str, probe: &Probe) -> Harness {
    harness_in(name, probe, None)
}

/// A window whose session file lives in `state_dir` (none: nothing is saved or restored).
pub(super) fn harness_in(name: &str, probe: &Probe, state_dir: Option<PathBuf>) -> Harness {
    harness_with(name, probe, state_dir, |_| {})
}

/// [`harness_in`] where `tweak` may change the window's surroundings first (the `PATH`, the
/// runtime folder).
pub(super) fn harness_with(
    name: &str,
    probe: &Probe,
    state_dir: Option<PathBuf>,
    tweak: impl FnOnce(&mut Ambient),
) -> Harness {
    harness_hosting(name, probe, state_dir, tweak, WindowHosting::Absent)
}

/// [`harness_with`] where the window also has a host that records the titles it is given
/// (`Harness::window_titles`).
pub(super) fn harness_hosting(
    name: &str,
    probe: &Probe,
    state_dir: Option<PathBuf>,
    tweak: impl FnOnce(&mut Ambient),
    hosting: WindowHosting,
) -> Harness {
    harness_building(name, probe, state_dir, tweak, hosting, |config| config)
}

/// [`harness_hosting`] where `extra` may add to the window's configuration (root contexts).
pub(super) fn harness_building(
    name: &str,
    probe: &Probe,
    state_dir: Option<PathBuf>,
    tweak: impl FnOnce(&mut Ambient),
    hosting: WindowHosting,
    extra: impl FnOnce(HarnessConfig) -> HarnessConfig,
) -> Harness {
    harness_at(name, probe, (state_dir, 100, None), tweak, hosting, extra)
}

/// A window whose first launch was asked to run `run` (`temor -e`).
pub(super) fn harness_running(name: &str, probe: &Probe, run: Run) -> Harness {
    harness_running_to(name, probe, run, RunEnds::default())
}

/// [`harness_running`] whose session file lives in `state_dir`.
pub(super) fn harness_running_in(
    name: &str,
    probe: &Probe,
    run: Run,
    state_dir: PathBuf,
) -> Harness {
    harness_at(
        name,
        probe,
        (Some(state_dir), 100, Some(run)),
        |_| {},
        WindowHosting::Absent,
        |config| config,
    )
}

/// [`harness_running`] on a desktop that tells `ends` when a run's tab closed.
pub(super) fn harness_running_to(name: &str, probe: &Probe, run: Run, ends: RunEnds) -> Harness {
    let desktop = Desktop {
        run_ends: ends,
        ..Desktop::none()
    };
    harness_at(
        name,
        probe,
        (None, 100, Some(run)),
        |_| {},
        WindowHosting::Absent,
        move |config| config.with_context(desktop),
    )
}

/// A window on a screen scaled `scale_percent` (100, 150, 200 ...).
pub(super) fn harness_scaled(name: &str, probe: &Probe, scale_percent: u16) -> Harness {
    harness_at(
        name,
        probe,
        (None, scale_percent, None),
        |_| {},
        WindowHosting::Absent,
        |config| config,
    )
}

/// The window the others are made from: `(state_dir, scale_percent, run)` are the session file,
/// the screen's scale and a program the first launch was asked to run.
fn harness_at(
    name: &str,
    probe: &Probe,
    (state_dir, scale_percent, run): (Option<PathBuf>, u16, Option<Run>),
    tweak: impl FnOnce(&mut Ambient),
    hosting: WindowHosting,
    extra: impl FnOnce(HarnessConfig) -> HarnessConfig,
) -> Harness {
    let home = scratch(name).join("home");
    // The person's own prompt, set where a person sets it: Debian and Ubuntu's bash reads
    // /etc/bash.bashrc first (through /etc/profile in a login shell), and its `user@host:path$ `
    // would replace the PS1 below. ~/.profile reads ~/.bashrc, as their default one does.
    std::fs::write(home.join(".bashrc"), "PS1='$ '\n").expect("scratch .bashrc");
    std::fs::write(home.join(".profile"), ". ~/.bashrc\n").expect("scratch .profile");
    let mut ambient = Ambient {
        home: home.clone(),
        env: [
            ("HOME", home.as_os_str()),
            ("PATH", std::ffi::OsStr::new("/usr/bin:/bin")),
            ("LANG", std::ffi::OsStr::new("C.UTF-8")),
            ("XDG_RUNTIME_DIR", home.join("xdg-runtime").as_os_str()),
            ("PS1", std::ffi::OsStr::new("$ ")),
            // The tests read the plain `$ ` prompt, so the shell keeps its own.
            ("TEMOR_PROMPT", std::ffi::OsStr::new("shell")),
        ]
        .into_iter()
        .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
        .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    tweak(&mut ambient);
    let launch = Launch {
        ambient,
        settings: TerminalSettings {
            shell: ShellChoice::Program(super::support::bash_path().display().to_string()),
            ..TerminalSettings::default()
        },
        appearance: AppearanceSource::scratch(home.join("config"), SystemPrefs::default()),
        // Our desktop: ⌘ is Super, or the Ctrl+Shift form the harness chords send.
        keymap: temor_ui::edge::keymap_for(
            chordkit::Platform::Linux {
                desktop: chordkit::Desktop::Ours,
            },
            &chordkit::Overrides::default(),
        )
        .0,
        launcher: LauncherChoice::Setsid(super::support::setsid_path()),
        probe: Some(probe.clone()),
        state_dir,
        first_words: Default::default(),
        first_run: TakeOnce::new(run),
    };
    // Wide enough that a pane splits side by side (two halves of at least 40 columns).
    let config = HarnessConfig::new(Viewport {
        width: 1024,
        height: 640,
        scale_percent,
    })
    .with_context(launch)
    .with_window_hosting(hosting);
    let mut h = Harness::new(App, extra(config));
    h.map();
    h
}

pub(super) fn wait_until(h: &mut Harness, probe: &Probe, what: &str, ok: impl Fn(&str) -> bool) {
    let start = Instant::now();
    while !ok(&probe.screen()) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            probe.screen()
        );
    }
}

#[test]
fn the_window_starts_a_shell_and_takes_typing() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-typing", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    // A click gives the pane the keyboard, as a person's click would.
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    for c in "echo window-ok".chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    wait_until(&mut h, &probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == "window-ok")
    });
}

/// The mouse reports on the screen, as `(button code, terminator)` for each `^[[<` report
/// cat -v shows. The button code is SGR's: 0 primary, 1 middle, 2 secondary.
fn mouse_reports(screen: &str) -> Vec<(u8, char)> {
    let mut out = Vec::new();
    for line in screen.lines() {
        let mut rest = line;
        while let Some(at) = rest.find("^[[<") {
            let after = &rest[at + 4..];
            let Some(end) = after.find(['M', 'm']) else {
                break;
            };
            let fields = &after[..end];
            let code = fields
                .split(';')
                .next()
                .and_then(|c| c.parse::<u8>().ok())
                .unwrap_or(u8::MAX);
            out.push((code, after[end..].chars().next().unwrap_or('?')));
            rest = &after[end + 1..];
        }
    }
    out
}

/// Turns on SGR mouse reporting and shows what the program receives, raw.
fn start_raw_mouse_echo(h: &mut Harness) {
    let command = "stty raw -echo; printf '\\033[?1000h\\033[?1006h'; cat -v";
    for c in command.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(500));
}

#[test]
fn a_click_reports_once_per_button() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-click", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let at = Point {
        x: Px(300.0),
        y: Px(200.0),
    };
    h.send(Input::click(at));
    h.advance(Duration::from_millis(100));
    start_raw_mouse_echo(&mut h);

    // Step 1, a primary click: one press and its release, each reported once.
    h.send(Input::click(at));
    h.advance(Duration::from_millis(500));
    let reports = mouse_reports(&probe.screen());
    let presses = reports.iter().filter(|(c, t)| *c == 0 && *t == 'M').count();
    let releases = reports.iter().filter(|(c, t)| *c == 0 && *t == 'm').count();
    assert_eq!(
        releases,
        1,
        "primary click: one primary release report, got {reports:?}:\n{}",
        probe.screen()
    );
    assert_eq!(
        presses, 1,
        "primary click: one primary press report, got {reports:?}"
    );

    // Step 2, a secondary click: one right press and release, and no new primary report.
    h.send(Input::button_down(at, PointerButton::Secondary));
    h.send(Input::button_up(at, PointerButton::Secondary));
    h.advance(Duration::from_millis(500));
    let reports = mouse_reports(&probe.screen());
    let presses = reports.iter().filter(|(c, t)| *c == 2 && *t == 'M').count();
    let releases = reports.iter().filter(|(c, t)| *c == 2 && *t == 'm').count();
    assert_eq!(
        presses,
        1,
        "secondary click: one right press report, got {reports:?}:\n{}",
        probe.screen()
    );
    assert_eq!(
        releases, 1,
        "secondary click: one right release report, got {reports:?}"
    );
    let primary = reports.iter().filter(|(c, _)| *c == 0).count();
    assert_eq!(
        primary, 2,
        "secondary click: no primary report of its own, got {reports:?}"
    );
}

/// Closing the find bar gives the keyboard back to the terminal: a key typed after the close
/// reaches the shell.
#[test]
fn closing_find_gives_the_keyboard_back_to_the_terminal() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-find-close", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('f'),
    ));
    h.advance(Duration::from_millis(200));
    assert!(h.html().contains("Find in terminal"), "find did not open");
    // Esc closes find and hands the keyboard back (the bar has no Done button).
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(200));
    for c in "echo after-find".chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    wait_until(&mut h, &probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == "after-find")
    });
}

/// How many tab rows the sidebar shows.
pub(super) fn rows(h: &Harness) -> usize {
    let html = h.html();
    html.matches("class=\"temor-tab-box\"").count() + html.matches("class=\"temor-pin\"").count()
}

/// How many terminal panes the tab in front shows.
pub(super) fn panes(h: &Harness) -> usize {
    h.html().matches("aria-label=\"Terminal\"").count()
}

pub(super) fn chord(h: &mut Harness, key: char) {
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char(key),
    ));
    h.advance(Duration::from_millis(300));
}

/// The ⌘ chords open and close tabs and panes: ⌘T then Enter a tab (⌘T is the command bar, and
/// its Enter on an empty bar is a plain tab), ⌘D a split, ⌘W a pane, and a chord nothing binds
/// never reaches the shell (^D would end it).
#[test]
fn the_command_chords_open_and_close_tabs_and_panes() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-tabs", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(200));
    assert_eq!((rows(&h), panes(&h)), (1, 1), "{}", h.html());

    chord(&mut h, 't');
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
    assert_eq!(rows(&h), 2, "a new tab:\n{}", h.html());
    chord(&mut h, 'd');
    assert_eq!((rows(&h), panes(&h)), (2, 2), "a split:\n{}", h.html());
    // An unbound chord: the shell must still be there afterwards.
    chord(&mut h, 'z');
    chord(&mut h, 'w');
    assert_eq!((rows(&h), panes(&h)), (2, 1), "one pane closed");
    chord(&mut h, 'w');
    assert_eq!((rows(&h), panes(&h)), (1, 1), "the tab closed");
    wait_until(&mut h, &probe, "the first tab's prompt", |s| {
        s.contains('$')
    });
}

/// The show/hide action (⌘⇧L, Super+Shift+L on our desktop) folds the sidebar away and brings it
/// back.
#[test]
fn the_sidebar_action_hides_and_shows_the_sidebar() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-sidebar-toggle", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(200));
    // A hidden sidebar keeps its rows in the document; its width is what goes.
    // Folded, the sidebar keeps its width inside a pane that clips it: the pane is what narrows.
    let sidebar_width = |h: &Harness| h.rect(".ds-split-pane").map_or(0.0, |r| r.size.width.0);
    assert!(sidebar_width(&h) > 100.0, "the sidebar starts shown");
    let toggle = |h: &mut Harness| {
        h.send(Input::chord(
            &[ShortcutKey::Super, ShortcutKey::Shift],
            ShortcutKey::Char('l'),
        ));
        h.advance(Duration::from_millis(600));
    };
    toggle(&mut h);
    assert!(sidebar_width(&h) < 1.0, "hidden: {}", sidebar_width(&h));
    toggle(&mut h);
    assert!(sidebar_width(&h) > 100.0, "shown again");
}

/// The shell ends, the person presses Restart, and the window carries on (Blitz once panicked in
/// `absolute_position` for a node that was gone). Frames keep running past the tooltip delay.
#[test]
fn restarting_an_ended_shell_gives_a_new_prompt_and_keeps_the_window_alive() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("window-restart", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    for c in "exit".chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.send(Input::key(ShortcutKey::Enter));
    let start = Instant::now();
    while !h.html().contains("The shell has ended.") {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the shell did not end"
        );
    }
    // The sidebar has a button of its own (New tab), so the banner's is found inside the banner.
    let restart = h
        .centre(".ds-inline-banner .ds-button")
        .unwrap_or_else(|| panic!("no Restart button:\n{}", h.html()));
    h.send(Input::click(restart));
    // Keep drawing well past the pointer's rest (a tooltip reads its anchor then).
    for _ in 0..60 {
        h.advance(Duration::from_millis(25));
    }
    assert!(!h.html().contains("The shell has ended."));
    wait_until(&mut h, &probe, "a new prompt", |s| s.contains('$'));
}

/// A saved session starts again: its tabs are in the sidebar and their shells run.
#[test]
fn a_saved_session_is_restored_on_launch() {
    if !super::support::tools_present() {
        return;
    }
    use temor_core::split::Direction;
    use temor_core::vocab::text::Cwd;
    use temor_core::workspace::{ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step};
    // The window's own scratch folder is emptied when it opens, so the session lives beside it.
    let state = scratch("window-restore-state").join("state");
    let here = Cwd(state.parent().expect("a parent").display().to_string());
    let ws = [
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
    temor_ui::edge::save_session(&state, &ws).expect("saved");
    let probe = Probe::new();
    let mut h = harness_in("window-restore", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(rows(&h), 2, "{}", h.html());
}

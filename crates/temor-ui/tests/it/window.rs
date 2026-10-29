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
use temor_ui::{AppearanceSource, Launch, TakeOnce};

use super::drive::{chord, focus_pane, panes, rows, started, type_line, wait_until};
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
        launcher: super::support::launcher_choice(),
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

#[test]
fn the_window_starts_a_shell_and_takes_typing() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-typing", &probe);
    // A click gives the pane the keyboard, as a person's click would.
    focus_pane(&mut h);
    type_line(&mut h, "echo window-ok");
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
    type_line(h, command);
    h.advance(Duration::from_millis(500));
}

#[test]
fn a_click_reports_once_per_button() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-click", &probe);
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
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-find-close", &probe);
    focus_pane(&mut h);
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('f'),
    ));
    h.advance(Duration::from_millis(200));
    assert!(h.html().contains("Find in terminal"), "find did not open");
    // Esc closes find and hands the keyboard back (the bar has no Done button).
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(200));
    type_line(&mut h, "echo after-find");
    wait_until(&mut h, &probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == "after-find")
    });
}

/// The ⌘ chords open and close tabs and panes: ⌘T then Enter a tab (⌘T is the command bar, and
/// its Enter on an empty bar is a plain tab), ⌘D a split, ⌘W a pane, and a chord nothing binds
/// never reaches the shell (^D would end it).
#[test]
fn the_command_chords_open_and_close_tabs_and_panes() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-tabs", &probe);
    focus_pane(&mut h);
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

/// The show/hide action (the standard Toggle Sidebar, ⌃⌘S as in mailo: Ctrl+Super+S on our
/// desktop) folds the sidebar away and brings it back.
#[test]
fn the_sidebar_action_hides_and_shows_the_sidebar() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-sidebar-toggle", &probe);
    focus_pane(&mut h);
    // A hidden sidebar keeps its rows in the document; its width is what goes.
    // Folded, the sidebar keeps its width inside a pane that clips it: the pane is what narrows.
    let sidebar_width = |h: &Harness| h.rect(".ds-split-pane").map_or(0.0, |r| r.size.width.0);
    assert!(sidebar_width(&h) > 100.0, "the sidebar starts shown");
    let toggle = |h: &mut Harness| {
        h.send(Input::chord(
            &[ShortcutKey::Ctrl, ShortcutKey::Super],
            ShortcutKey::Char('s'),
        ));
        h.advance(Duration::from_millis(600));
    };
    toggle(&mut h);
    assert!(sidebar_width(&h) < 1.0, "hidden: {}", sidebar_width(&h));
    toggle(&mut h);
    assert!(sidebar_width(&h) > 100.0, "shown again");
}

/// The shell ends and its tab closes, and the window carries on (Blitz once panicked in
/// `absolute_position` for a node that was gone). Frames keep running past the tooltip delay.
#[test]
fn an_exited_shell_closes_its_tab_and_keeps_the_window_alive() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-exit-closes", &probe);
    // A second tab, so the window stays when the first one ends.
    chord(&mut h, 'n');
    assert_eq!(rows(&h), 2, "two tabs");
    focus_pane(&mut h);
    for c in "exit".chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.send(Input::key(ShortcutKey::Enter));
    let start = Instant::now();
    while rows(&h) != 1 {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the tab did not close"
        );
    }
    // Keep drawing well past the pointer's rest (a tooltip reads its anchor then).
    for _ in 0..60 {
        h.advance(Duration::from_millis(25));
    }
    assert!(!h.html().contains("The shell has ended."));
    wait_until(&mut h, &probe, "the first tab's prompt", |s| {
        s.contains('$')
    });
}

/// A saved session starts again: its tabs are in the sidebar and their shells run.
#[test]
fn a_saved_session_is_restored_on_launch() {
    if !super::support::window_tools_present() {
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
    temor_client::keep::older::save(&state, &ws).expect("saved");
    let probe = Probe::new();
    let mut h = harness_in("window-restore", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(rows(&h), 2, "{}", h.html());
}

/// The rows are kept in the store: a session file an older temor left is read in once, and what
/// the window keeps after that is what comes back, not the file.
#[test]
fn the_rows_are_kept_in_the_store_after_the_old_file_is_read_in_once() {
    if !super::support::window_tools_present() {
        return;
    }
    use temor_core::vocab::text::Cwd;
    use temor_core::workspace::{ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step};
    let state = scratch("window-layout-state").join("state");
    let here = Cwd(state.parent().expect("a parent").display().to_string());
    let ws = [
        WorkspaceIn::NewTab {
            cwd: Some(here.clone()),
        },
        WorkspaceIn::NewTab { cwd: Some(here) },
    ]
    .into_iter()
    .fold(
        Workspace::new(ClosedState::new(KeepClosed::DEFAULT)),
        |ws, step| workspace_step(ws, step).0,
    );
    temor_client::keep::older::save(&state, &ws).expect("saved");
    let probe = Probe::new();
    let mut h = harness_in("window-layout", &probe, Some(state.clone()));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(rows(&h), 2, "the old file's tabs: {}", h.html());
    focus_pane(&mut h);
    chord(&mut h, 'w');
    let start = Instant::now();
    while rows(&h) != 1 {
        h.advance(Duration::from_millis(25));
        assert!(start.elapsed() < Duration::from_secs(10), "{}", h.html());
    }
    drop(h);
    assert!(
        state.join("session.json").is_file(),
        "the old file is left where it was"
    );

    let probe = Probe::new();
    let mut h = harness_in("window-layout-again", &probe, Some(state));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    for _ in 0..20 {
        h.advance(Duration::from_millis(25));
    }
    assert_eq!(rows(&h), 1, "what was kept, not the file: {}", h.html());
}

#[test]
fn clicking_the_new_tab_row_opens_a_tab() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("window-new-tab-row", &probe);
    assert_eq!(rows(&h), 1, "one tab");
    let at = super::drive::rects(&h, ".temor-side .temor-new-tab .ds-row")[0].centre();
    h.send(Input::click(at));
    h.advance(Duration::from_millis(400));
    assert_eq!(rows(&h), 2, "the New Tab row opened a tab");
}

/// A shell that ends before its first prompt (here its rc file exits) keeps its tab and says so:
/// a new tab would end the same way, so closing it would only hide why.
#[test]
fn a_shell_that_ends_before_its_first_prompt_keeps_its_tab_and_the_banner() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness_with("window-ends-at-once", &probe, None, |ambient| {
        std::fs::write(ambient.home.join(".bashrc"), "exit 3\n").expect("bashrc");
    });
    let start = Instant::now();
    while !h.html().contains("The shell has ended.") {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the shell did not end:\n{}",
            h.html()
        );
    }
    for _ in 0..40 {
        h.advance(Duration::from_millis(25));
    }
    assert_eq!(rows(&h), 1, "the tab stays");
    assert!(h.html().contains("The shell has ended."));
}

/// A screen kept at quit comes back with its commands as cards, a failed one still saying how it
/// ended: the marks are found again from the screen and the kept commands.
#[test]
fn a_restored_screen_shows_its_commands_as_cards_with_their_status() {
    if !super::support::tools_present() {
        return;
    }
    let state = scratch("window-restore-cards-state").join("state");
    let probe = Probe::new();
    let mut h = harness_in("window-restore-cards", &probe, Some(state.clone()));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "echo kept-one");
    wait_until(&mut h, &probe, "the output", |s| {
        s.lines().any(|l| l.trim_end() == "kept-one")
    });
    type_line(&mut h, "echo kept-two; (exit 4)");
    let start = Instant::now();
    while !h.html().contains(">exit 4") {
        h.advance(Duration::from_millis(25));
        assert!(start.elapsed() < Duration::from_secs(10), "{}", h.html());
    }
    let before = h
        .html()
        .matches("class=\"temor-bseg temor-blk-head\"")
        .count();
    drop(h);

    let probe = Probe::new();
    let mut h = harness_in("window-restore-cards-again", &probe, Some(state));
    let start = Instant::now();
    loop {
        let html = h.html();
        let cards = html.matches("class=\"temor-bseg temor-blk-head\"").count();
        if cards >= before && html.contains(">exit 4") {
            break;
        }
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{cards} cards after the restart, {before} before:\n{}\n{}",
            probe.screen(),
            html.split("class=\"temor-bseg temor-blk-head")
                .skip(1)
                .map(|c| c.chars().take(400).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n---\n")
        );
    }
}

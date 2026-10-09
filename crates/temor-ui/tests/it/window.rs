//! The window in ds-harness: a real Blitz document with the app's root, driven by the events a
//! window would deliver (never OS input). The pane's text is read from the probe the window
//! publishes after each repaint, since its pixels are a texture.

use std::time::{Duration, Instant};

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::Input;
use ds_harness::{Driver, Harness, HarnessConfig, Viewport};
use temor_pane::settings::ShellChoice;
use temor_pane::settings::TerminalSettings;
use temor_ui::edge::Ambient;
use temor_ui::view::App;
use temor_ui::view::probe::Probe;
use temor_ui::{Launch, LauncherChoice};

use super::support::scratch;

fn harness(name: &str, probe: &Probe) -> Harness {
    let home = scratch(name).join("home");
    let ambient = Ambient {
        home: home.clone(),
        env: [
            ("HOME", home.as_os_str()),
            ("PATH", std::ffi::OsStr::new("/usr/bin:/bin")),
            ("LANG", std::ffi::OsStr::new("C.UTF-8")),
            ("XDG_RUNTIME_DIR", home.join("xdg-runtime").as_os_str()),
            ("PS1", std::ffi::OsStr::new("$ ")),
        ]
        .into_iter()
        .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
        .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    let launch = Launch {
        ambient,
        settings: TerminalSettings {
            shell: ShellChoice::Program(super::support::bash_path().display().to_string()),
            ..TerminalSettings::default()
        },
        launcher: LauncherChoice::Setsid(
            temor_pty::find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin"))
                .expect("util-linux setsid is installed"),
        ),
        probe: Some(probe.clone()),
    };
    let config = HarnessConfig::new(Viewport {
        width: 760,
        height: 484,
        scale_percent: 100,
    })
    .with_context(launch);
    let mut h = Harness::new(App, config);
    h.map();
    h
}

fn wait_until(h: &mut Harness, probe: &Probe, what: &str, ok: impl Fn(&str) -> bool) {
    let start = Instant::now();
    while !ok(&probe.screen()) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(15),
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

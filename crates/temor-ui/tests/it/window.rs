//! The window in ds-harness: a real Blitz document with the app's root, driven by the events a
//! window would deliver (never OS input). The pane's text is read from the probe the window
//! publishes after each repaint, since its pixels are a texture.

use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::Input;
use ds_harness::{Driver, Harness, HarnessConfig, Viewport};
use temor_core::terminal::settings::TerminalSettings;
use temor_core::terminal::settings_value::ShellChoice;
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
            shell: ShellChoice::Program("/usr/bin/bash".to_owned()),
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

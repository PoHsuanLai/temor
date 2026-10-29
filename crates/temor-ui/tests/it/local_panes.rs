//! A pane in the local daemon: the shell runs in a process that outlives the window, so a second
//! window over the same state finds the same shell (same process, same screen); and a running
//! program can be detached into a pane of its own while the old pane gets a new shell.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds_harness::Input;
use temor_remote::host::HostEnv;
use temor_ui::edge::Ambient;
use temor_ui::edge::remote_conn::dial_local;
use temor_ui::view::probe::Probe;

use super::drive::{chord, focus_pane, panes, rects, type_line, type_text, wait_until};
use super::support::{scratch, window_tools_present};
use super::window::harness_with;

/// The helper the build made beside the test programs, if it has been built.
fn helper() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let at = exe.parent()?.parent()?.join("temor-remote");
    at.is_file().then_some(at)
}

/// The number after `label=` on a line of the screen.
fn number_after(screen: &str, label: &str) -> Option<u64> {
    screen
        .lines()
        .filter_map(|line| line.trim().strip_prefix(label))
        .find_map(|rest| rest.trim().parse().ok())
}

/// A daemon of its own for one test: a short runtime folder (a socket's path is short on macOS;
/// the system temp folder there is not), and the helper. Its sessions end with it.
struct Rig {
    helper: PathBuf,
    run: tempfile::TempDir,
}

impl Rig {
    fn new() -> Option<Self> {
        let helper = helper().filter(|_| window_tools_present())?;
        let run = tempfile::Builder::new()
            .prefix("tmr-")
            .tempdir_in("/tmp")
            .expect("a runtime folder");
        Some(Self { helper, run })
    }

    fn tweak(&self, ambient: &mut Ambient) {
        for (key, value) in [
            ("TEMOR_LOCAL_DAEMON", "1".into()),
            ("TEMOR_REMOTE_HELPER", self.helper.clone().into_os_string()),
            ("TEMOR_REMOTE_LINGER", "2".into()),
            ("XDG_RUNTIME_DIR", self.run.path().as_os_str().to_owned()),
        ] {
            ambient.env.set(key, value);
        }
    }
}

impl Drop for Rig {
    /// Leaves nothing running: the sessions end and the unused daemon exits after its linger.
    fn drop(&mut self) {
        let env = HostEnv {
            xdg_runtime_dir: Some(self.run.path().into()),
            home: None,
            shell: None,
            linger: Some("2".into()),
            exe: self.helper.clone(),
        };
        if let Ok((conn, _events)) = dial_local(&env)
            && let Ok(listed) = conn.client().list_sessions()
        {
            for session in listed {
                let _ = conn.client().close_session(session.session);
            }
        }
    }
}

#[test]
fn a_local_pane_keeps_its_shell_when_the_window_quits_and_the_next_window_finds_it() {
    let Some(rig) = Rig::new() else {
        return;
    };
    let state = scratch("local-panes").join("state");

    let probe = Probe::new();
    let mut first = harness_with("local-panes", &probe, Some(state.clone()), |a| rig.tweak(a));
    wait_until(&mut first, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut first);
    type_line(&mut first, "echo pid=$$");
    wait_until(&mut first, &probe, "the pid", |s| {
        number_after(s, "pid=").is_some()
    });
    let before = number_after(&probe.screen(), "pid=").expect("a pid");
    drop(first);

    let probe = Probe::new();
    let mut second = harness_with("local-panes-again", &probe, Some(state), |a| rig.tweak(a));
    wait_until(&mut second, &probe, "the old screen", |s| {
        number_after(s, "pid=") == Some(before)
    });
    focus_pane(&mut second);
    type_line(&mut second, "echo same=$$");
    wait_until(&mut second, &probe, "the pid again", |s| {
        number_after(s, "same=").is_some()
    });
    assert_eq!(number_after(&probe.screen(), "same="), Some(before));
}

#[test]
fn a_running_program_is_detached_into_its_own_pane_and_the_old_pane_gets_a_new_shell() {
    let Some(rig) = Rig::new() else {
        return;
    };
    let probe = Probe::new();
    let mut h = harness_with("local-detach", &probe, None, |a| rig.tweak(a));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "echo shell=$$; sleep 300");
    wait_until(&mut h, &probe, "the shell's pid", |s| {
        number_after(s, "shell=").is_some()
    });
    let old = number_after(&probe.screen(), "shell=").expect("a pid");
    h.advance(Duration::from_millis(500));

    chord(&mut h, 't');
    type_text(&mut h, "detach");
    h.advance(Duration::from_millis(300));
    h.send(Input::key(ShortcutKey::Enter));
    let until = Instant::now() + Duration::from_secs(20);
    while panes(&h) != 2 {
        assert!(
            Instant::now() < until,
            "no second pane:\n{}",
            probe.screen()
        );
        h.advance(Duration::from_millis(50));
    }
    // The new pane has the keys and shows the program that was running.
    wait_until(&mut h, &probe, "the parked program's screen", |s| {
        number_after(s, "shell=") == Some(old)
    });

    // The old pane has a new shell: another process, same folder. A click gives it the keys.
    let left = rects(&h, ".temor-pane")
        .into_iter()
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .expect("a pane");
    h.send(Input::click(left.centre()));
    h.advance(Duration::from_millis(500));
    wait_until(&mut h, &probe, "a prompt in the old pane", |s| {
        s.contains('$') && number_after(s, "shell=") != Some(old)
    });
    type_line(&mut h, "echo fresh=$$");
    wait_until(&mut h, &probe, "the new shell's pid", |s| {
        number_after(s, "fresh=").is_some()
    });
    assert_ne!(number_after(&probe.screen(), "fresh="), Some(old));
}

/// The centre of each element `selector` finds, top to bottom and left to right.
fn centres(h: &ds_harness::Harness, selector: &str) -> Vec<ds::prelude::Point> {
    let mut found = rects(h, selector);
    found.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    found.into_iter().map(|r| r.centre()).collect()
}

/// Steps the window until `ok` holds for its page, or panics with what it shows.
fn wait_html(h: &mut ds_harness::Harness, what: &str, ok: impl Fn(&str) -> bool) {
    let until = Instant::now() + Duration::from_secs(20);
    while !ok(&h.html()) {
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}:\n{}",
            h.html()
        );
        h.advance(Duration::from_millis(50));
    }
}

/// The block menu moves a running program to a live card: a pane of its own with a head that
/// names the command. Stop interrupts it and hangs the session up after the grace; the card keeps
/// its last frame until it is put away, and then its pane goes.
#[test]
fn a_running_program_moves_to_a_live_card_that_stops_and_is_put_away() {
    let Some(rig) = Rig::new() else {
        return;
    };
    let probe = Probe::new();
    let mut h = harness_with("local-live-card", &probe, None, |a| rig.tweak(a));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_line(&mut h, "echo before; (exit 3)");
    wait_html(&mut h, "the first command's status", |html| {
        html.contains(">exit 3")
    });
    type_line(&mut h, "echo live=$$; sleep 300");
    wait_until(&mut h, &probe, "the shell's pid", |s| {
        number_after(s, "live=").is_some()
    });
    // Its head, not a count of heads: the sticky head over the list is a head too.
    wait_html(&mut h, "the running card", |html| {
        html.contains("temor-bseg temor-blk-head\" data-state=\"running\"")
    });

    // The running block's menu offers to detach it.
    let card = *centres(&h, ".temor-blk-head").last().unwrap();
    h.send(Input::press(
        ds::prelude::Point {
            x: ds::prelude::Px(300.0),
            y: card.y,
        },
        ds::base::press::PointerButton::Secondary,
    ));
    h.advance(Duration::from_millis(300));
    assert!(
        h.html().contains("Detach Running Program"),
        "no Detach in the menu:\n{}",
        h.html()
    );
    let items = centres(&h, ".ds-menu-item");
    h.send(Input::click(items[6]));
    wait_html(&mut h, "the live card's head", |html| {
        html.contains("class=\"temor-live-head\"") && html.contains("echo live=$$; sleep 300")
    });
    assert_eq!(panes(&h), 2);
    // The command before it came along on the card's screen, with how it ended.
    wait_html(&mut h, "the earlier command's status on the card", |html| {
        html.contains(">exit 3")
    });
    assert!(
        h.html().contains("data-state=\"running\""),
        "the card runs:\n{}",
        h.html()
    );

    // Size, Full, Stop.
    let icons = centres(&h, ".temor-live-head .ds-button");
    assert_eq!(icons.len(), 3, "{}", h.html());
    h.send(Input::click(icons[2]));
    // The interrupt ends `sleep`; the session's shell is hung up when the grace is over.
    wait_html(&mut h, "the card to end", |html| {
        html.contains("class=\"temor-live-head\" data-state=\"ended\"")
            || html.contains("data-state=\"ended\" class=\"temor-live-head\"")
    });
    let icons = centres(&h, ".temor-live-head .ds-button");
    assert_eq!(icons.len(), 1, "only Put away:\n{}", h.html());
    h.send(Input::click(icons[0]));
    let until = Instant::now() + Duration::from_secs(10);
    while panes(&h) != 1 {
        assert!(
            Instant::now() < until,
            "the card's pane stays:\n{}",
            h.html()
        );
        h.advance(Duration::from_millis(50));
    }
    assert!(!h.html().contains("class=\"temor-live-head"));
}

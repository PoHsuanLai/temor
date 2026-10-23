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

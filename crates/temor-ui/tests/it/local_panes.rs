//! A pane in the local daemon: the shell runs in a process that outlives the window, so a second
//! window over the same state finds the same shell (same process, same screen).

use std::path::PathBuf;

use temor_remote::host::HostEnv;
use temor_ui::edge::remote_conn::dial_local;
use temor_ui::view::probe::Probe;

use super::drive::{focus_pane, type_line, wait_until};
use super::support::{bash_path, scratch, window_tools_present};
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

#[test]
fn a_local_pane_keeps_its_shell_when_the_window_quits_and_the_next_window_finds_it() {
    let Some(helper) = helper().filter(|_| window_tools_present()) else {
        return;
    };
    let state = scratch("local-panes").join("state");
    // A socket's path is short on macOS; the system temp folder there is not.
    let run = tempfile::Builder::new()
        .prefix("tmr-")
        .tempdir_in("/tmp")
        .expect("a runtime folder");
    let tweak = |ambient: &mut temor_ui::edge::Ambient| {
        for (key, value) in [
            ("TEMOR_LOCAL_DAEMON", "1".into()),
            ("TEMOR_REMOTE_HELPER", helper.clone().into_os_string()),
            ("TEMOR_REMOTE_LINGER", "2".into()),
            ("XDG_RUNTIME_DIR", run.path().as_os_str().to_owned()),
            ("SHELL", bash_path().into_os_string()),
        ] {
            ambient.env.set(key, value);
        }
    };

    let probe = Probe::new();
    let mut first = harness_with("local-panes", &probe, Some(state.clone()), tweak);
    wait_until(&mut first, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut first);
    type_line(&mut first, "echo pid=$$");
    wait_until(&mut first, &probe, "the pid", |s| {
        number_after(s, "pid=").is_some()
    });
    let before = number_after(&probe.screen(), "pid=").expect("a pid");
    drop(first);

    let probe = Probe::new();
    let mut second = harness_with("local-panes-again", &probe, Some(state), tweak);
    wait_until(&mut second, &probe, "the old screen", |s| {
        number_after(s, "pid=") == Some(before)
    });
    focus_pane(&mut second);
    type_line(&mut second, "echo same=$$");
    wait_until(&mut second, &probe, "the pid again", |s| {
        number_after(s, "same=").is_some()
    });
    assert_eq!(number_after(&probe.screen(), "same="), Some(before));
    drop(second);

    // Leave nothing running: the sessions end and the unused daemon exits after its linger.
    let env = HostEnv {
        xdg_runtime_dir: Some(run.path().into()),
        home: None,
        shell: None,
        linger: Some("2".into()),
        exe: helper,
    };
    if let Ok((conn, _events)) = dial_local(&env)
        && let Ok(listed) = conn.client().list_sessions()
    {
        for session in listed {
            let _ = conn.client().close_session(session.session);
        }
    }
}

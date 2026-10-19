//! The pane sockets at the window's edge: reports from a program reach the window tagged with
//! its pane, the socket is private, and a restarted pane gets a working socket again. Scratch
//! runtime folder only.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use temor_pane::report::{ReportState, StatusLine, encode_hello, encode_status};
use temor_pty::SessionLauncher;
use temor_ui::app::shell_pane_with_socket;
use temor_ui::edge::{Ambient, SocketHost};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::support::{bash_path, scratch, tools_present};

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

/// Says hello, sends one status and returns the replies.
fn report(socket: &Path, line: &StatusLine) -> (String, String) {
    let stream = UnixStream::connect(socket).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    writer.write_all(encode_hello().as_bytes()).expect("hello");
    let mut hello = String::new();
    reader.read_line(&mut hello).expect("hello reply");
    writer
        .write_all(encode_status(line).as_bytes())
        .expect("status");
    let mut status = String::new();
    reader.read_line(&mut status).expect("status reply");
    (hello, status)
}

fn working(seq: u64, note: &str) -> StatusLine {
    StatusLine {
        seq,
        state: ReportState::Working,
        wait: None,
        app: Some("claude".to_owned()),
        note: Some(note.to_owned()),
    }
}

/// The next report the window hears, with the pane it came from.
fn heard(events: &mut UnboundedReceiver<(PaneId, PaneEvent)>) -> (PaneId, StatusLine) {
    let start = Instant::now();
    loop {
        if let Ok((pane, PaneEvent::Reported(line))) = events.try_recv() {
            return (pane, line);
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "no report arrived"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_until_gone(path: &Path) {
    let start = Instant::now();
    while path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{} stayed",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "the private agent socket is Linux-only so far"
)]
fn a_programs_report_reaches_the_window_tagged_with_its_pane_over_a_private_socket() {
    let runtime = scratch("ag-sock").join("run");
    std::fs::create_dir_all(&runtime).expect("runtime");
    let (tx, mut events) = unbounded_channel();
    let host = SocketHost::start(runtime.clone(), tx).expect("host");

    let seven = host.open(PaneId(7)).expect("socket 7");
    let eight = host.open(PaneId(8)).expect("socket 8");
    assert_eq!(seven, runtime.join("7.sock"));
    assert!(seven.as_os_str().len() < 104);
    assert_eq!((mode(&seven), mode(&runtime)), (0o600, 0o700));

    let (hello, status) = report(&seven, &working(1, "reading the code"));
    assert_eq!((hello.trim(), status.trim()), ("ok 1", "ok 1"));
    let (pane, line) = heard(&mut events);
    assert_eq!(pane, PaneId(7));
    assert_eq!(
        (line.state, line.note.as_deref()),
        (ReportState::Working, Some("reading the code"))
    );

    report(&eight, &working(1, "other"));
    assert_eq!(heard(&mut events).0, PaneId(8));

    // A pane closed has no socket; the same pane started again has a new, working one.
    host.close(PaneId(7));
    wait_until_gone(&seven);
    let again = host.open(PaneId(7)).expect("socket 7 again");
    report(&again, &working(1, "restarted"));
    assert_eq!(heard(&mut events).0, PaneId(7));

    drop(host);
    wait_until_gone(&eight);
}

#[test]
fn a_shell_is_told_where_its_socket_is() {
    if !tools_present() {
        return;
    }
    let home = scratch("ag-env").join("home");
    let ambient = Ambient {
        home: home.clone(),
        env: [
            ("HOME", home.as_os_str()),
            ("PATH", std::ffi::OsStr::new("/usr/bin:/bin")),
        ]
        .into_iter()
        .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
        .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    let settings = temor_pane::settings::TerminalSettings {
        shell: temor_pane::settings::ShellChoice::Program(bash_path().display().to_string()),
        ..temor_pane::settings::TerminalSettings::default()
    };
    let launcher = SessionLauncher::Setsid(
        temor_pty::find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin")).expect("setsid"),
    );
    let spec_for = |socket: Option<std::path::PathBuf>| {
        shell_pane_with_socket(
            &ambient,
            &settings,
            3,
            (
                temor_vt::GridSize::new(80, 24),
                temor_vt::CellPx::new(8, 16),
            ),
            temor_render::Palette::default(),
            launcher.clone(),
            socket,
        )
        .expect("spec")
    };
    let with = spec_for(Some(home.join("runtime/3.sock")));
    let without = spec_for(None);
    assert_eq!(
        with.spawn.env.get("TEMOR_SOCKET"),
        Some(home.join("runtime/3.sock").as_os_str())
    );
    assert_eq!(
        with.spawn.env.get("TEMOR_PANE_ID"),
        Some(std::ffi::OsStr::new("3"))
    );
    assert_eq!(without.spawn.env.get("TEMOR_SOCKET"), None);
}

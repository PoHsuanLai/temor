//! Shared helpers: a scratch HOME, a bash started on a pane, and reading the screen back.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use temor_pane::{PaneEvent, PaneHandle, PaneSpec, start};
use temor_pty::{SessionLauncher, SpawnSpec, find_program};
use temor_render::Palette;
use temor_shell::ChildEnv;
use temor_vt::{CellPx, GridSize, LineCount};

/// Where tests put their files: under the build directory, never under the real HOME.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("home")).expect("scratch home");
    dir
}

/// A bash that reads no rc files, in `home`, with a fixed prompt.
pub fn bash_spec(home: &Path, grid: GridSize, cell: CellPx) -> PaneSpec {
    let mut env = ChildEnv::new();
    env.set("HOME", home);
    env.set("PATH", "/usr/bin:/bin");
    env.set("TERM", "xterm-256color");
    env.set("LANG", "C.UTF-8");
    env.set("PS1", "$ ");
    env.set("HISTFILE", home.join("history"));
    let spawn = SpawnSpec {
        program: bash_path(),
        args: ["--norc", "--noprofile", "-i"]
            .into_iter()
            .map(Into::into)
            .collect(),
        cwd: home.to_path_buf(),
        env,
        size: temor_pty::GridSize::cells(
            temor_pty::Cols(grid.cols.0),
            temor_pty::Rows(grid.rows.0),
        ),
    };
    let setsid = find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin"))
        .expect("util-linux setsid is installed");
    PaneSpec {
        spawn,
        launcher: SessionLauncher::Setsid(setsid),
        grid,
        cell,
        scrollback: LineCount(1000),
        palette: Palette::default(),
        cursor: temor_vt::CursorShape::Block,
    }
}

/// A started pane and the channel its events arrive on.
pub struct Started {
    pub pane: PaneHandle,
    pub events: Receiver<PaneEvent>,
}

pub fn spawn_bash(home: &Path, cols: u16, rows: u16) -> Started {
    let (tx, events) = channel();
    let grid = GridSize::new(cols, rows);
    let (pane, runner) =
        start(bash_spec(home, grid, CellPx::new(8, 16)), Arc::new(tx)).expect("pane starts");
    std::thread::spawn(move || runner.run());
    Started { pane, events }
}

/// Every screen row, joined with newlines.
pub fn screen(pane: &PaneHandle) -> String {
    pane.with_term(|term| {
        let rows = term.screen_rows().unwrap_or(0);
        (0..rows)
            .map(|row| {
                term.row_text(row)
                    .map(|line| line.text().trim_end().to_owned())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Waits until the screen satisfies `ok`, or panics with the screen as it was.
pub fn wait_for(started: &mut Started, what: &str, ok: impl Fn(&str) -> bool) -> String {
    let start = Instant::now();
    loop {
        drain(&mut started.events);
        let text = screen(&started.pane);
        if ok(&text) {
            return text;
        }
        if start.elapsed() > Duration::from_secs(8) {
            panic!("timed out waiting for {what}; screen:\n{text}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Consumes pending events; returns whether the screen was reported dirty.
pub fn drain(events: &mut Receiver<PaneEvent>) -> bool {
    let mut dirty = false;
    while let Ok(event) = events.try_recv() {
        dirty |= matches!(event, PaneEvent::Dirty);
    }
    dirty
}

/// Whether the tools these tests drive are present: bash, util-linux `setsid`, and `/proc`.
/// Where one is missing the test returns early (with a note) instead of failing.
pub fn tools_present() -> bool {
    let path = std::ffi::OsStr::new("/usr/bin:/bin");
    let found = temor_pty::find_program("bash", path).is_some()
        && temor_pty::find_program("setsid", path).is_some()
        && std::path::Path::new("/proc/self").exists();
    if !found {
        // TEMOR_REQUIRE_TOOLS=1 (set by the lane's gate on a machine that has the tools) turns the
        // skip into a failure, so the tests cannot pass silently there.
        if std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1") {
            panic!("bash, setsid and /proc are required (TEMOR_REQUIRE_TOOLS=1)");
        }
        eprintln!(
            "SKIPPED: needs bash, setsid and /proc (set TEMOR_REQUIRE_TOOLS=1 to fail instead)"
        );
    }
    found
}

/// Counts the events waiting on the channel, consuming them.
pub fn count_events(events: &mut Receiver<PaneEvent>) -> usize {
    let mut count = 0;
    while let Ok(event) = events.try_recv() {
        // The foreground process's name is read on a timer, not driven by output.
        if !matches!(event, PaneEvent::Foreground(_)) {
            count += 1;
        }
    }
    count
}

/// Where bash is, from the standard places (the tests skip when it is not there).
pub fn bash_path() -> PathBuf {
    temor_pty::find_program("bash", std::ffi::OsStr::new("/usr/bin:/bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/bash"))
}

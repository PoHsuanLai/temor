//! Shared helpers for the block tests: a session on a real shell in a scratch home, a fake clock
//! and notifier, and reading the blocks back.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use temor_core::block::Outcome;
use temor_pane::PaneEvent;
use temor_pane::keys::Keymap;
use temor_pane::settings::NotifyAfter;
use temor_pane::settings::ShellChoice;
use temor_pane::settings::TerminalSettings;
use temor_pty::SessionLauncher;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, PixelSize, Scale};
use temor_ui::app::shell_pane;
use temor_ui::edge::{Ambient, FakeClock, RecordingNotifier};
use temor_ui::platform::{OpenError, Opener};
use temor_ui::session::{Clipboard, ClipboardFailed, Session, SessionParts};
use tokio::sync::mpsc::UnboundedReceiver;

use super::super::support::scratch;

/// A clipboard the test can read.
#[derive(Clone, Default)]
pub(super) struct MemoryClipboard(pub(super) Rc<RefCell<String>>);

impl Clipboard for MemoryClipboard {
    fn write(&self, text: &str) -> Result<(), ClipboardFailed> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }

    fn read(&self) -> Result<String, ClipboardFailed> {
        Ok(self.0.borrow().clone())
    }
}

#[derive(Debug, Default)]
pub(super) struct NoOpener;

impl Opener for NoOpener {
    fn open(&self, _target: &temor_pane::text::LinkTarget) -> Result<(), OpenError> {
        Ok(())
    }
}

/// One session on `shell`, with its clock, notifier and event channel in the test's hands.
pub(super) struct Live {
    /// The shelf folder the session saves into.
    pub(super) shelf: PathBuf,
    /// A file whose appearance lets a held command finish (see `hold`).
    pub(super) go: PathBuf,
    pub(super) session: Session,
    pub(super) events: UnboundedReceiver<PaneEvent>,
    pub(super) clock: FakeClock,
    pub(super) notifier: RecordingNotifier,
    pub(super) clipboard: MemoryClipboard,
}

/// Where a shell is, if the test can run it: PATH, then the shells the lanes keep.
pub(super) fn shell_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into());
    temor_pty::find_program(name, &path).or_else(|| {
        let shelf = PathBuf::from("/mnt/data/cargo/pty-temor/shells/bin").join(name);
        shelf.is_file().then_some(shelf)
    })
}

pub(super) fn start(name: &str, shell: &std::path::Path) -> Live {
    start_with(name, shell, |_| {})
}

/// Starts a session like `start`, after `setup` has written the scratch home.
pub(super) fn start_with(
    name: &str,
    shell: &std::path::Path,
    setup: impl FnOnce(&std::path::Path),
) -> Live {
    // One scratch call per test: a second one would remove the shell's own working folder.
    let base = scratch(name);
    let home = base.join("home");
    std::fs::create_dir_all(&home).expect("home");
    setup(&home);
    let ambient = Ambient {
        home: home.clone(),
        env: [
            ("HOME", home.as_os_str()),
            ("PATH", std::ffi::OsStr::new("/usr/bin:/bin:/usr/local/bin")),
            ("LANG", std::ffi::OsStr::new("C.UTF-8")),
            ("XDG_CONFIG_HOME", home.join("config").as_os_str()),
            ("XDG_DATA_HOME", home.join("data").as_os_str()),
            ("XDG_RUNTIME_DIR", home.join("xdg-runtime").as_os_str()),
        ]
        .into_iter()
        .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
        .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    let settings = TerminalSettings {
        shell: ShellChoice::Program(shell.display().to_string()),
        ..TerminalSettings::default()
    };
    let setsid =
        temor_pty::find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin")).expect("setsid");
    let spec = shell_pane(
        &ambient,
        &settings,
        1,
        temor_vt::GridSize::new(80, 24),
        temor_vt::CellPx::new(8, 16),
        temor_render::Palette::default(),
        SessionLauncher::Setsid(setsid),
    )
    .expect("shell spec");
    let fonts = FontSet::load(
        &FontConfig {
            family: FontFamily::Monospace,
            size: FontSize::new(13.0),
        },
        FontSource::System,
    )
    .expect("a monospace font");
    let (clock, notifier, clipboard) = (
        FakeClock::at(1_000),
        RecordingNotifier::default(),
        MemoryClipboard::default(),
    );
    let (tx, events) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::open(
        SessionParts {
            spec,
            fonts,
            keymap: Keymap::defaults(),
            clipboard: Box::new(clipboard.clone()),
            opener: Box::new(NoOpener),
            notify_after: NotifyAfter::DEFAULT,
            notifier: Box::new(notifier.clone()),
            clock: Box::new(clock.clone()),
            shelf_dir: home.join("shelf"),
        },
        tx,
    )
    .expect("session opens");
    let mut live = Live {
        shelf: home.join("shelf"),
        go: base.join("go"),
        session,
        events,
        clock,
        notifier,
        clipboard,
    };
    live.session
        .layout(PixelSize::new(640, 384), Scale::new(1.0))
        .expect("layout");
    pump(&mut live, "the first prompt", |s| s.prompts_seen() > 0);
    live
}

/// Feeds the session the events its pane sent, until `done` holds.
/// Absorbs events until none has come for `quiet` (at most ten seconds), so a mark the shell sends
/// late lands before a test counts anchors.
pub(super) fn settle(live: &mut Live, quiet: Duration) {
    let start = Instant::now();
    let mut last = Instant::now();
    while last.elapsed() < quiet && start.elapsed() < Duration::from_secs(10) {
        match live.events.try_recv() {
            Ok(event) => {
                live.session.absorb(event);
                last = Instant::now();
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

pub(super) fn pump(live: &mut Live, what: &str, done: impl Fn(&Session) -> bool) {
    let start = Instant::now();
    loop {
        while let Ok(event) = live.events.try_recv() {
            live.session.absorb(event);
        }
        if done(&live.session) {
            return;
        }
        assert!(
            // Generous: shells start slowly on a loaded machine, and the wait ends as soon as it holds.
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}; screen:\n{}",
            live.session.screen_text()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Runs a command that stays open until `go` exists, so the test can move the fake clock while
/// it runs. Any earlier `go` file is removed first.
pub(super) fn hold(live: &Live) {
    let _ = std::fs::remove_file(&live.go);
    run_line(
        live,
        &format!(
            "while [ ! -e '{}' ]; do sleep 0.05; done; true",
            live.go.display()
        ),
    );
}

/// Types a line into the shell and presses Enter.
pub(super) fn run_line(live: &Live, line: &str) {
    live.session.pane().send(format!("{line}\r").as_bytes());
}

/// The blocks, as (command, outcome), oldest first.
pub(super) fn blocks(live: &Live) -> Vec<(String, Outcome)> {
    live.session
        .block_outcomes()
        .into_iter()
        .map(|(id, outcome)| {
            (
                live.session
                    .block_command(id)
                    .unwrap_or_default()
                    .to_owned(),
                outcome,
            )
        })
        .collect()
}

/// Every shell the machine has, with a note for each one it lacks.
pub(super) fn shells() -> Vec<(&'static str, PathBuf)> {
    ["bash", "zsh", "fish", "nu"]
        .into_iter()
        .filter_map(|name| match shell_path(name) {
            Some(path) => Some((name, path)),
            None => {
                println!("note: block marks for {name} skipped: {name} is not installed");
                None
            }
        })
        .collect()
}

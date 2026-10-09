//! Command blocks in a real session (PLAN §9): real shells mark their commands, and the blocks
//! come out with the right output and exit status. Bash always runs; zsh, fish and nushell run
//! where they are installed (a note says so when one is not). The gutter, the prompt jumps and the
//! finish notice are driven by direct calls, on a fake clock and a recording notifier.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ds_core::time::stamp::Stamp;
use temor_core::block::Outcome;
use temor_core::notify::{Focus, NotifyAfter};
use temor_core::terminal::action::Action;
use temor_core::terminal::chord::Chord;
use temor_core::terminal::keymap::Keymap;
use temor_core::terminal::settings::TerminalSettings;
use temor_core::terminal::settings_value::ShellChoice;
use temor_core::vocab::ids::BlockId;
use temor_pty::SessionLauncher;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, PixelSize, Scale};
use temor_ui::app::shell_pane;
use temor_ui::edge::{Ambient, FakeClock, RecordingNotifier};
use temor_ui::input::Stroke;
use temor_ui::pane::PaneEvent;
use temor_ui::platform::{OpenError, Opener};
use temor_ui::session::{Clipboard, ClipboardFailed, Session, SessionParts};
use temor_vt::{KeyCode, KeyInput, Modifiers, Notification};
use tokio::sync::mpsc::UnboundedReceiver;

use super::support::scratch;

/// A clipboard the test can read.
#[derive(Clone, Default)]
struct MemoryClipboard(Rc<RefCell<String>>);

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
struct NoOpener;

impl Opener for NoOpener {
    fn open(&self, _target: &temor_core::terminal::link::LinkTarget) -> Result<(), OpenError> {
        Ok(())
    }
}

/// One session on `shell`, with its clock, notifier and event channel in the test's hands.
struct Live {
    /// A file whose appearance lets a held command finish (see `hold`).
    go: PathBuf,
    session: Session,
    events: UnboundedReceiver<PaneEvent>,
    clock: FakeClock,
    notifier: RecordingNotifier,
    clipboard: MemoryClipboard,
}

/// Where a shell is, if the test can run it: PATH, then the shells the lanes keep.
fn shell_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into());
    temor_pty::find_program(name, &path).or_else(|| {
        let shelf = PathBuf::from("/mnt/data/cargo/pty-temor/shells/bin").join(name);
        shelf.is_file().then_some(shelf)
    })
}

fn start(name: &str, shell: &std::path::Path) -> Live {
    // One scratch call per test: a second one would remove the shell's own working folder.
    let base = scratch(name);
    let home = base.join("home");
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
fn pump(live: &mut Live, what: &str, done: impl Fn(&Session) -> bool) {
    let start = Instant::now();
    loop {
        while let Ok(event) = live.events.try_recv() {
            live.session.absorb(event);
        }
        if done(&live.session) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for {what}; screen:\n{}",
            live.session.screen_text()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Runs a command that stays open until `go` exists, so the test can move the fake clock while
/// it runs. Any earlier `go` file is removed first.
fn hold(live: &Live) {
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
fn run_line(live: &Live, line: &str) {
    live.session.pane().send(format!("{line}\r").as_bytes());
}

/// The blocks, as (command, outcome), oldest first.
fn blocks(live: &Live) -> Vec<(String, Outcome)> {
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
fn shells() -> Vec<(&'static str, PathBuf)> {
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

#[test]
fn real_shells_make_one_block_per_command_with_its_output_and_status() {
    if !super::support::tools_present() {
        return;
    }
    for (name, shell) in shells() {
        let mut live = start(&format!("blocks-{name}"), &shell);
        run_line(&live, "echo hello-block");
        pump(&mut live, "the first block", |s| {
            s.block_outcomes().len() == 1 && s.block_outcomes()[0].1 != Outcome::Running
        });
        run_line(&live, "sh -c \"exit 1\"");
        pump(&mut live, "the second block", |s| {
            s.block_outcomes().len() == 2 && s.block_outcomes()[1].1 != Outcome::Running
        });
        let found = blocks(&live);
        assert_eq!(
            found,
            vec![
                ("echo hello-block".to_owned(), Outcome::Succeeded),
                (
                    "sh -c \"exit 1\"".to_owned(),
                    Outcome::Failed(temor_core::vocab::ids::ExitCode(1))
                ),
            ],
            "{name}"
        );
        let first = live.session.block_outcomes()[0].0;
        assert_eq!(
            live.session.block_output(first).as_deref(),
            Some("hello-block"),
            "{name}: the output between the marks"
        );
    }
}

#[test]
fn a_gutter_press_selects_the_block_output_and_copies_it() {
    if !super::support::tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-gutter", &shell);
    run_line(&live, "echo one; echo two");
    pump(&mut live, "the block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let marks = live.session.block_marks();
    let mark = marks.first().expect("the prompt is on screen");
    let fit = live.session.fit().expect("fitted");
    let (_, y) = fit.cell_origin(temor_render::CellPos::new(
        0,
        u16::try_from(mark.row).unwrap(),
    ));
    let at = temor_vt::SurfacePoint {
        x: 1,
        y: y.0 as i32 + 1,
    };
    assert_eq!(
        live.session.gutter_block(at),
        Some(mark.id),
        "the gutter is the block's"
    );
    assert!(
        live.session.gutter_press(at).expect("press"),
        "the gutter took the press"
    );
    assert_eq!(live.session.selected_block(), Some(mark.id));
    live.session.run(Action::Copy).expect("copy");
    assert_eq!(live.clipboard.0.borrow().as_str(), "one\ntwo");
    // A press in the grid (not the gutter) is not a gutter press, and clears the block.
    let grid = temor_vt::SurfacePoint {
        x: 40,
        y: y.0 as i32 + 1,
    };
    assert!(!live.session.gutter_press(grid).expect("press"));
}

#[test]
fn control_shift_up_and_down_move_between_prompts() {
    if !super::support::tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-jump", &shell);
    for _ in 0..3 {
        run_line(&live, "seq 1 30");
        let before = live.session.block_outcomes().len();
        pump(&mut live, "a finished block", |s| {
            s.block_outcomes().len() > before
                && s.block_outcomes()
                    .last()
                    .is_some_and(|b| b.1 != Outcome::Running)
        });
    }
    let prompt_top = |s: &Session| s.block_marks().iter().find(|m| m.row == 0).map(|m| m.id);
    let up = Stroke {
        chord: Some(Chord::parse("ctrl+shift+up").expect("chord")),
        key: KeyInput::press(KeyCode::ArrowUp, Modifiers::CTRL.with(Modifiers::SHIFT), ""),
    };
    let down = Stroke {
        chord: Some(Chord::parse("ctrl+shift+down").expect("chord")),
        key: KeyInput::press(
            KeyCode::ArrowDown,
            Modifiers::CTRL.with(Modifiers::SHIFT),
            "",
        ),
    };
    let ids: Vec<BlockId> = live
        .session
        .block_outcomes()
        .into_iter()
        .map(|b| b.0)
        .collect();
    live.session.key(up.clone()).expect("up");
    assert_eq!(
        prompt_top(&live.session),
        Some(ids[2]),
        "the newest prompt above the live screen"
    );
    live.session.key(up).expect("up again");
    assert_eq!(prompt_top(&live.session), Some(ids[1]));
    live.session.key(down).expect("down");
    assert_eq!(prompt_top(&live.session), Some(ids[2]));
}

#[test]
fn a_long_command_notifies_once_when_the_pane_is_unfocused() {
    if !super::support::tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-notify", &shell);
    live.session.set_focus(Focus::Unfocused);
    hold(&live);
    pump(&mut live, "the command to start", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    // Twelve seconds pass on the fake clock while the command runs.
    live.clock.advance(12_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the command to finish", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let sent = live.notifier.sent();
    assert_eq!(sent.len(), 1, "one notice: {sent:?}");
    assert_eq!(sent[0].title, "Command finished");
    assert!(
        sent[0].body.ends_with("finished after 12.0 s"),
        "{:?}",
        sent[0].body
    );
}

#[test]
fn a_long_command_while_focused_and_a_short_one_while_unfocused_stay_quiet() {
    if !super::support::tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-quiet", &shell);
    live.session.set_focus(Focus::Focused);
    hold(&live);
    pump(&mut live, "the focused command to start", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    live.clock.advance(60_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the focused command to finish", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    live.session.set_focus(Focus::Unfocused);
    hold(&live);
    pump(&mut live, "the short command to start", |s| {
        s.block_outcomes()
            .get(1)
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    live.clock.advance(2_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the short command to finish", |s| {
        s.block_outcomes()
            .get(1)
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    assert!(live.notifier.sent().is_empty());
}

#[test]
fn program_notifications_and_the_bell_reach_the_edge_rate_limited() {
    let shell = shell_path("bash").unwrap_or_else(|| PathBuf::from("/usr/bin/bash"));
    if !super::support::tools_present() {
        return;
    }
    let mut live = start("blocks-program", &shell);
    live.session.set_focus(Focus::Unfocused);
    let note = |body: &str| {
        PaneEvent::Notification(Notification {
            title: String::new(),
            body: body.to_owned(),
        })
    };
    live.session.absorb(note("first"));
    live.session.absorb(note("too soon"));
    live.clock.advance(2_000);
    live.session.absorb(note("later"));
    live.session.absorb(PaneEvent::Bell);
    let sent = live.notifier.sent();
    assert_eq!(
        sent.iter().map(|n| n.body.as_str()).collect::<Vec<_>>(),
        vec!["first", "later"],
        "one per two seconds"
    );
    assert!(live.session.bell_showing(), "the bell's mark is up");
    live.clock.advance(1_000);
    assert!(!live.session.bell_showing(), "and goes down on its own");
    let _ = Stamp(0);
}

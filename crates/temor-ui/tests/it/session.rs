//! The session as the window drives it, with a real bash on a PTY: keys, a resize, selection and
//! copy, paste (framed, and confirmed when unsafe), and the find bar.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use temor_core::terminal::action::Action;
use temor_core::terminal::chord::ChordKey;
use temor_core::terminal::chord::{Chord, Mods};
use temor_core::terminal::keymap::Keymap;
use temor_core::terminal::paste::UnsafeReason;
use temor_core::terminal::selection::CellPos;
use temor_core::terminal::settings::TerminalSettings;
use temor_core::terminal::settings_value::ShellChoice;
use temor_pty::SessionLauncher;
use temor_render::CellPos as PaintCell;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, PixelSize, Scale};
use temor_ui::app::shell_pane;
use temor_ui::edge::Ambient;
use temor_ui::input::{FindBar, Stroke, route};
use temor_ui::pane::PaneEvent;
use temor_ui::platform::{OpenError, Opener};
use temor_ui::session::{Clipboard, ClipboardFailed, Session, SessionParts};
use temor_vt::{KeyAction, KeyCode, KeyInput, Modifiers, SurfacePoint};

use super::support::scratch;

/// A clipboard shared between the test and the session.
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

struct Fixture {
    session: Session,
    clipboard: MemoryClipboard,
    _events: tokio::sync::mpsc::UnboundedReceiver<PaneEvent>,
}

fn fixture(name: &str) -> Fixture {
    let home = scratch(name).join("home");
    let ambient = Ambient {
        home: home.clone(),
        env: [
            ("HOME", home.as_os_str()),
            ("PATH", std::ffi::OsStr::new("/usr/bin:/bin")),
            ("LANG", std::ffi::OsStr::new("C.UTF-8")),
            ("XDG_RUNTIME_DIR", home.join("xdg-runtime").as_os_str()),
        ]
        .into_iter()
        .map(|(k, v)| (std::ffi::OsString::from(k), v.to_os_string()))
        .collect(),
        config_dir: home.join("config"),
        runtime_dir: home.join("runtime"),
    };
    let settings = TerminalSettings {
        shell: ShellChoice::Program(super::support::bash_path().display().to_string()),
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
    let clipboard = MemoryClipboard::default();
    let (tx, events) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::open(
        SessionParts {
            spec,
            fonts,
            keymap: Keymap::defaults(),
            clipboard: Box::new(clipboard.clone()),
            opener: Box::new(NoOpener),
        },
        tx,
    )
    .expect("session opens");
    Fixture {
        session,
        clipboard,
        _events: events,
    }
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn has_line(text: &str, line: &str) -> bool {
    // Blank cells before a line's text are spaces now, and a raw terminal's cursor may start a
    // line part way along, so the match ignores the blanks at both ends.
    text.lines().any(|l| l.trim() == line.trim())
}

fn type_text(session: &mut Session, text: &str) {
    for c in text.chars() {
        let code = match c {
            ' ' => KeyCode::Space,
            c => key_code(c),
        };
        let input = KeyInput::press(code, Modifiers::NONE, &c.to_string());
        session
            .key(Stroke {
                chord: None,
                key: input,
            })
            .expect("key");
    }
}

fn press_enter(session: &mut Session) {
    let input = KeyInput::press(KeyCode::Enter, Modifiers::NONE, "");
    session
        .key(Stroke {
            chord: None,
            key: input,
        })
        .expect("enter");
}

fn key_code(c: char) -> KeyCode {
    match c.to_ascii_lowercase() {
        'a' => KeyCode::A,
        'b' => KeyCode::B,
        'c' => KeyCode::C,
        'd' => KeyCode::D,
        'e' => KeyCode::E,
        'f' => KeyCode::F,
        'g' => KeyCode::G,
        'h' => KeyCode::H,
        'i' => KeyCode::I,
        'k' => KeyCode::K,
        'l' => KeyCode::L,
        'm' => KeyCode::M,
        'n' => KeyCode::N,
        'o' => KeyCode::O,
        'p' => KeyCode::P,
        'r' => KeyCode::R,
        's' => KeyCode::S,
        't' => KeyCode::T,
        'u' => KeyCode::U,
        'w' => KeyCode::W,
        'x' => KeyCode::X,
        'y' => KeyCode::Y,
        '-' => KeyCode::Minus,
        '$' => KeyCode::Digit4,
        _ => KeyCode::Space,
    }
}

/// Starts a session at a fixed pixel size and waits for the first prompt.
fn ready(name: &str) -> Fixture {
    let mut f = fixture(name);
    f.session
        .layout(PixelSize::new(800, 480), Scale::new(1.0))
        .expect("layout");
    wait_until("the prompt", || f.session.screen_text().contains('$'));
    f
}

/// The viewport point at the middle of a cell.
fn point_at(session: &Session, row: u16, col: u16) -> SurfacePoint {
    let fit = session.fit().expect("fitted");
    let (x, y) = fit.cell_origin(PaintCell::new(col, row));
    SurfacePoint {
        x: x.0 as i32 + 1,
        y: y.0 as i32 + 1,
    }
}

#[test]
fn typing_reaches_the_program_and_its_output_is_drawn() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-typing");
    type_text(&mut f.session, "echo typed-ok");
    press_enter(&mut f.session);
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "typed-ok")
    });
}

#[test]
fn a_resize_refits_the_grid_and_the_program_size() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-resize");
    let first = f.session.fit().expect("fitted");
    let first_want = format!("{} {}", first.grid.rows.0, first.grid.cols.0);
    type_text(&mut f.session, "stty size");
    press_enter(&mut f.session);
    wait_until("the first size", || {
        has_line(&f.session.screen_text(), &first_want)
    });
    let changed = f
        .session
        .layout(PixelSize::new(1200, 720), Scale::new(1.0))
        .expect("layout");
    assert!(changed, "a bigger pane changes the grid");
    let fit = f.session.fit().expect("fitted");
    let want = format!("{} {}", fit.grid.rows.0, fit.grid.cols.0);
    // The program learns its size on the I/O thread, a moment after the layout; ask until the
    // answer shows it.
    let start = Instant::now();
    loop {
        type_text(&mut f.session, "stty size");
        press_enter(&mut f.session);
        std::thread::sleep(Duration::from_millis(200));
        if has_line(&f.session.screen_text(), &want) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "the program never saw {want}"
        );
    }
    assert!(
        f.session
            .layout(PixelSize::new(1200, 720), Scale::new(1.0))
            .is_ok_and(|c| !c)
    );
}

#[test]
fn selecting_a_word_and_copying_puts_it_on_the_clipboard() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-copy");
    type_text(&mut f.session, "echo copied-word");
    press_enter(&mut f.session);
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "copied-word")
    });
    let row = f
        .session
        .screen_text()
        .lines()
        .position(|l| l.trim_end() == "copied-word")
        .expect("the output row") as u16;
    let at = point_at(&f.session, row, 2);
    let mods = Modifiers::NONE;
    f.session
        .press(
            at,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_secs(1),
        )
        .expect("press");
    f.session
        .press(
            at,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_millis(1100),
        )
        .expect("double click");
    f.session
        .release(at, temor_vt::MouseButton::Left, mods)
        .expect("release");
    f.session.run(Action::Copy).expect("copy");
    // A word ends at the hyphen: word characters are letters, digits and underscores.
    assert_eq!(*f.clipboard.0.borrow(), "copied");
}

#[test]
fn a_paste_is_framed_and_an_unsafe_one_waits_for_the_person() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-paste");
    type_text(&mut f.session, "echo bracketed");
    press_enter(&mut f.session);
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "bracketed")
    });
    // bash turns bracketed paste on at its prompt.
    wait_until("bracketed paste", || {
        f.session.pane().with_term(|t| t.paste_mode())
            == temor_core::terminal::paste::PasteMode::Bracketed
    });
    *f.clipboard.0.borrow_mut() = "echo pasted-ok".to_owned();
    f.session.run(Action::Paste).expect("paste");
    press_enter(&mut f.session);
    wait_until("the pasted command's output", || {
        has_line(&f.session.screen_text(), "pasted-ok")
    });

    // Bash has bracketed paste on, so a line break is framed and safe: no confirmation.
    *f.clipboard.0.borrow_mut() = "echo framed-lines\necho framed-2".to_owned();
    f.session.run(Action::Paste).expect("paste");
    assert!(f.session.pending_paste().is_none());
    // The end of a frame could leave it, so that paste waits for the person.
    *f.clipboard.0.borrow_mut() = "x\x1b[201~echo never".to_owned();
    f.session.run(Action::Paste).expect("paste");
    let pending = f
        .session
        .pending_paste()
        .cloned()
        .expect("an unsafe paste waits");
    assert_eq!(pending.reason, UnsafeReason::EndOfFrame);
    f.session.cancel_paste();
    assert!(f.session.pending_paste().is_none());
    // A plain paste (no bracketed paste) with a line break waits too.
    let plain = temor_core::terminal::paste::check(
        "a\nb",
        temor_core::terminal::paste::PasteMode::Plain,
        temor_core::terminal::paste::VtVerdict::Clean,
    );
    assert_eq!(
        plain,
        temor_core::terminal::paste::PasteCheck::NeedsConfirmation(UnsafeReason::Newline)
    );
}

#[test]
fn the_find_bar_finds_and_steps_through_matches() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-find");
    type_text(&mut f.session, "echo needle; echo needle");
    press_enter(&mut f.session);
    wait_until("both outputs", || {
        f.session.screen_text().matches("needle").count() >= 4
    });
    f.session.run(Action::Find).expect("open find");
    f.session.set_query("needle").expect("search");
    let finder = f.session.finder().expect("open");
    let total = finder.position().map(|(_, n)| n).expect("matches");
    assert!(
        total >= 4,
        "the command line and both outputs match: {total}"
    );
    f.session.run(Action::FindNext).expect("next");
    assert_eq!(
        f.session
            .finder()
            .and_then(|f| f.position())
            .map(|(i, _)| i),
        Some(2)
    );
    f.session.run(Action::FindPrevious).expect("previous");
    f.session.run(Action::FindPrevious).expect("previous wraps");
    assert_eq!(
        f.session
            .finder()
            .and_then(|f| f.position())
            .map(|(i, _)| i),
        Some(total)
    );
    f.session.run(Action::Copy).expect("copy the match");
    assert_eq!(*f.clipboard.0.borrow(), "needle");
    f.session.run(Action::CloseFind).expect("close");
    assert!(f.session.finder().is_none());
}

#[test]
fn the_find_keys_belong_to_the_bar_only_while_it_is_open() {
    if !super::support::tools_present() {
        return;
    }
    let keymap = Keymap::defaults();
    let next = Chord::parse("ctrl+shift+g").expect("valid");
    assert_eq!(keymap.action_for(&next), Some(Action::FindNext));
    let stroke = Stroke {
        chord: Some(next),
        key: KeyInput {
            code: KeyCode::G,
            action: KeyAction::Press,
            mods: Modifiers::CTRL.with(Modifiers::SHIFT),
            text: String::new(),
            unshifted: Some('g'),
            ime: temor_vt::Ime::Idle,
        },
    };
    assert_eq!(
        route(stroke, &keymap, FindBar::Closed),
        temor_ui::input::Routed::Ignored
    );
    let _ = (
        ChordKey::Char('g'),
        Mods::NONE,
        CellPos::new(0, 0),
        PathBuf::new(),
    );
}

#[test]
fn the_pane_paints_its_text_into_pixels() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-paint");
    type_text(&mut f.session, "echo painted");
    press_enter(&mut f.session);
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "painted")
    });
    let mut painted = None;
    f.session
        .paint(|bytes, size| {
            let colours: std::collections::HashSet<&[u8]> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| px.as_slice())
                .collect();
            painted = Some((bytes.len(), size.width.0, size.height.0, colours.len()));
        })
        .expect("paint");
    let (len, width, height, colours) = painted.expect("the pane has a size, so it paints");
    assert_eq!(
        len,
        width as usize * height as usize * 4,
        "premultiplied RGBA8, one row after another"
    );
    // Glyph edges antialias, so more than the background and the ink colours appear.
    assert!(colours > 2, "text was drawn: {colours} distinct colours");
}

#[test]
fn a_find_query_runs_only_after_typing_pauses() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-debounce");
    f.session.queue_query("prompt", Duration::from_millis(0));
    let early = f
        .session
        .run_due_query(Duration::from_millis(100))
        .expect("check");
    assert!(!early, "no search ran while typing");
    assert!(f.session.finder().is_none());
    // The newest keystroke restarts the quiet period.
    f.session.queue_query("prompt2", Duration::from_millis(120));
    let restarted = f
        .session
        .run_due_query(Duration::from_millis(200))
        .expect("check");
    assert!(
        !restarted,
        "the quiet period restarted at the last keystroke"
    );
    let ran = f
        .session
        .run_due_query(Duration::from_millis(300))
        .expect("check");
    assert!(ran);
    assert_eq!(f.session.find_text(), "prompt2");
    assert!(f.session.finder().is_some());
}

#[test]
fn a_paste_over_the_limit_is_refused_with_a_notice_and_never_cut() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-paste-limit");
    *f.clipboard.0.borrow_mut() = "x".repeat(temor_ui::session::PASTE_LIMIT + 1);
    f.session.run(Action::Paste).expect("paste");
    let notice = f
        .session
        .take_notice()
        .expect("a notice explains the refusal");
    assert!(notice.contains("not pasted"), "{notice}");
    assert!(
        f.session.pending_paste().is_none(),
        "nothing is queued to send"
    );
}

#[test]
fn a_five_mebibyte_paste_arrives_intact() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-big-paste");
    // Raw mode lets dd take every byte; it counts the bytes and the bytes outside the alphabet.
    type_text(
        &mut f.session,
        "stty raw -echo; echo READY; dd of=out bs=65536 count=80 iflag=fullblock 2>/dev/null; echo $(wc -c < out) $(tr -d '0-9a-z' < out | wc -c)",
    );
    press_enter(&mut f.session);
    wait_until("the reader is ready", || {
        has_line(&f.session.screen_text(), "READY")
    });
    wait_until("plain paste", || {
        f.session.pane().with_term(|t| t.paste_mode())
            == temor_core::terminal::paste::PasteMode::Plain
    });
    let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let data: String = (0..5 * 1024 * 1024)
        .map(|i| char::from(alphabet[i % alphabet.len()]))
        .collect();
    *f.clipboard.0.borrow_mut() = data;
    f.session.run(Action::Paste).expect("paste");
    wait_until("five mebibytes and no stray bytes", || {
        has_line(&f.session.screen_text(), "5242880 0")
    });
}

#[test]
fn wide_characters_copy_and_select_by_cells() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-cjk");
    f.session.pane().send("echo 中文ab\r".as_bytes());
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "中文ab")
    });
    let row = f
        .session
        .screen_text()
        .lines()
        .position(|l| l.trim_end() == "中文ab")
        .expect("the output row") as u16;
    // Cells 0 to 3 hold 中 and 文 (two columns each).
    let mods = Modifiers::NONE;
    let start = point_at(&f.session, row, 0);
    f.session
        .press(
            start,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_secs(1),
        )
        .expect("press");
    f.session
        .drag_to(point_at(&f.session, row, 3))
        .expect("drag");
    f.session
        .release(
            point_at(&f.session, row, 3),
            temor_vt::MouseButton::Left,
            mods,
        )
        .expect("release");
    f.session.run(Action::Copy).expect("copy");
    assert_eq!(*f.clipboard.0.borrow(), "中文");
    // Double-clicking the a (cell 4) selects its word. Ideographs are letters, so the word runs
    // back over the two wide characters: the copy is the whole run, cell by cell.
    let at = point_at(&f.session, row, 4);
    f.session
        .press(
            at,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_secs(2),
        )
        .expect("press");
    f.session
        .press(
            at,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_millis(2100),
        )
        .expect("double");
    f.session
        .release(at, temor_vt::MouseButton::Left, mods)
        .expect("release");
    f.session.run(Action::Copy).expect("copy");
    assert_eq!(*f.clipboard.0.borrow(), "中文ab");
}

#[test]
fn a_tab_copies_as_blanks_and_search_finds_the_blanks() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-tab");
    // Write straight into the terminal: cursor home, clear the line, then "a", a tab, "b".
    f.session.pane().with_term(|t| {
        let _ = t.write(b"\x1b[H\x1b[2Ka\tb");
    });
    wait_until("the row", || {
        f.session
            .screen_text()
            .lines()
            .next()
            .is_some_and(|l| l.starts_with("a"))
    });
    // A tab from column 1 reaches column 8: 'b' sits in column 8, after seven blank cells.
    let at = point_at(&f.session, 0, 0);
    let mods = Modifiers::NONE;
    f.session
        .press(
            at,
            temor_vt::MouseButton::Left,
            mods,
            Duration::from_secs(1),
        )
        .expect("press");
    f.session.drag_to(point_at(&f.session, 0, 8)).expect("drag");
    f.session
        .release(
            point_at(&f.session, 0, 8),
            temor_vt::MouseButton::Left,
            mods,
        )
        .expect("release");
    f.session.run(Action::Copy).expect("copy");
    assert_eq!(*f.clipboard.0.borrow(), "a       b");
    // Searching for the same run finds it; two literal blanks are text too.
    f.session.set_query("a       b").expect("search");
    assert_eq!(f.session.finder().map(|f| f.matches().len()), Some(1));
    f.session.pane().with_term(|t| {
        let _ = t.write(b"\x1b[2;1H\x1b[2Ka  b");
    });
    f.session.set_query("a  b").expect("search");
    assert_eq!(f.session.finder().map(|f| f.matches().len()), Some(1));
}

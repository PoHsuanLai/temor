//! The session as the window drives it, with a real bash on a PTY: keys, a resize, selection and
//! copy, paste (framed, and confirmed when unsafe), and the find bar.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use chordkit::{AltGr, Context, Desktop, Key as ChordKey, Modifier, Modifiers as ChordMods};
use chordkit::{Overrides, Platform};
use temor_pane::PaneEvent;
use temor_pane::keys::Action;
use temor_pane::paste::UnsafeReason;
use temor_pane::settings::ShellChoice;
use temor_pane::settings::TerminalSettings;
use temor_pty::SessionLauncher;
use temor_render::CellPos as PaintCell;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, PixelSize, Scale};
use temor_ui::app::shell_pane;
use temor_ui::edge::{Ambient, keymap_for};
use temor_ui::input::{FindBar, Routed, Stroke, route};
use temor_ui::platform::{OpenError, Opener};
use temor_ui::session::{Clipboard, ClipboardFailed, Session, SessionKit, SessionParts};
use temor_vt::{KeyAction, KeyCode, KeyInput, Modifiers, SurfacePoint};

use super::support::scratch;

/// A clipboard shared between the test and the session, and its primary selection.
#[derive(Clone, Default)]
struct MemoryClipboard(Rc<RefCell<String>>, Rc<RefCell<String>>);

impl Clipboard for MemoryClipboard {
    fn write(&self, text: &str) -> Result<(), ClipboardFailed> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }

    fn read(&self) -> Result<String, ClipboardFailed> {
        Ok(self.0.borrow().clone())
    }

    fn write_primary(&self, text: &str) -> Result<(), ClipboardFailed> {
        *self.1.borrow_mut() = text.to_owned();
        Ok(())
    }
}

#[derive(Debug, Default)]
struct NoOpener;

impl Opener for NoOpener {
    fn open(&self, _target: &temor_pane::text::LinkTarget) -> Result<(), OpenError> {
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
            // The tests wait for the system's `$` prompt, so the shell keeps its own.
            ("TEMOR_PROMPT", std::ffi::OsStr::new("shell")),
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
    let setsid = super::support::setsid_path();
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
            ..FontConfig::default()
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
            kit: SessionKit {
                keymap: keymap_for(OURS, &Overrides::default()).0,
                clipboard: Box::new(clipboard.clone()),
                opener: Box::new(NoOpener),
                notify_after: TerminalSettings::default().notify_after,
                cursor_blink: TerminalSettings::default().cursor_blink,
                notifier: Box::new(temor_ui::edge::NoopNotifier),
                clock: Box::new(temor_ui::edge::SystemClock::new()),
                shelf_dir: home.join("shelf"),
            },
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
            start.elapsed() < super::support::SLOW_WAIT,
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
                input: None,
                key: input,
            })
            .expect("key");
    }
}

fn press_enter(session: &mut Session) {
    let input = KeyInput::press(KeyCode::Enter, Modifiers::NONE, "");
    session
        .key(Stroke {
            input: None,
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
fn a_plain_click_leaves_nothing_selected() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-click");
    type_text(&mut f.session, "echo clicked-word");
    press_enter(&mut f.session);
    wait_until("the output", || {
        has_line(&f.session.screen_text(), "clicked-word")
    });
    let row = f
        .session
        .screen_text()
        .lines()
        .position(|l| l.trim_end() == "clicked-word")
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
        .release(at, temor_vt::MouseButton::Left, mods)
        .expect("release");
    *f.clipboard.0.borrow_mut() = "before".to_owned();
    f.session.run(Action::Copy).expect("copy");
    assert_eq!(*f.clipboard.0.borrow(), "before", "a click copies nothing");
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
        f.session.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Bracketed
    });
    *f.clipboard.0.borrow_mut() = "echo pasted-ok".to_owned();
    f.session.run(Action::Paste).expect("paste");
    press_enter(&mut f.session);
    wait_until("the pasted command's output", || {
        has_line(&f.session.screen_text(), "pasted-ok")
    });
    // bash turns bracketed paste off while a command runs and back on at its next prompt.
    wait_until("bracketed paste again", || {
        f.session.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Bracketed
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
    let plain = temor_pane::paste::check(
        "a\nb",
        temor_pane::paste::PasteMode::Plain,
        temor_pane::paste::VtVerdict::Clean,
    );
    assert_eq!(
        plain,
        temor_pane::paste::PasteCheck::NeedsConfirmation(UnsafeReason::Newline)
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

/// The platform these tests run the keys on: our desktop, where ⌘ is Super or Ctrl+Shift.
const OURS: Platform = Platform::Linux {
    desktop: Desktop::Ours,
};

/// A chord as chordkit reads a press: these modifiers and this character.
fn chord(mods: &[Modifier], c: char) -> chordkit::KeyInput {
    chordkit::KeyInput::new(
        mods.iter().copied().collect::<ChordMods>(),
        ChordKey::Char(c),
    )
}

#[test]
fn the_find_keys_belong_to_the_bar_only_while_it_is_open() {
    if !super::support::tools_present() {
        return;
    }
    let keymap = keymap_for(OURS, &Overrides::default()).0;
    let next = chord(&[Modifier::Ctrl, Modifier::Shift], 'g');
    assert_eq!(
        keymap
            .resolve(&next, Context::Terminal)
            .and_then(|named| Action::from_chordkit(&named)),
        Some(Action::FindNext)
    );
    let stroke = Stroke {
        input: Some(next),
        key: KeyInput {
            code: KeyCode::G,
            action: KeyAction::Press,
            mods: Modifiers::CTRL.with(Modifiers::SHIFT),
            text: String::new(),
            unshifted: Some('g'),
            ime: temor_vt::Ime::Idle,
        },
    };
    assert_eq!(route(stroke, &keymap, FindBar::Closed), Routed::Ignored);
}

/// A key press as the window routes it: the press as chordkit reads it (when it has one) and
/// what the program gets.
fn pressed(
    input: Option<chordkit::KeyInput>,
    code: KeyCode,
    mods: Modifiers,
    text: &str,
) -> Stroke {
    Stroke {
        input,
        key: KeyInput::press(code, mods, text),
    }
}

#[test]
fn ctrl_c_reaches_the_program_as_an_interrupt_and_altgr_text_as_text() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-keys-rows");
    // Ctrl+C is not a chord the pane runs: bash echoes the interrupt it received as ^C.
    f.session
        .key(pressed(
            Some(chord(&[Modifier::Ctrl], 'c')),
            KeyCode::C,
            Modifiers::CTRL,
            "",
        ))
        .expect("ctrl+c");
    wait_until("the interrupt echoed", || {
        f.session.screen_text().contains("^C")
    });
    // AltGr types a character as Ctrl+Alt with text: the program gets the text.
    type_text(&mut f.session, "echo ");
    f.session
        .key(pressed(
            Some(chord(&[Modifier::Ctrl, Modifier::Alt], '2').with_alt_gr(AltGr::Held)),
            KeyCode::Digit2,
            Modifiers::CTRL.with(Modifiers::ALT),
            "@",
        ))
        .expect("altgr");
    press_enter(&mut f.session);
    wait_until("the altgr text as the program's output", || {
        has_line(&f.session.screen_text(), "@")
    });
}

#[test]
fn command_v_pastes_in_both_command_forms() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-paste-forms");
    for (text, chord_mods, mods) in [
        (
            "echo pasted-ctrl-shift",
            vec![Modifier::Ctrl, Modifier::Shift],
            Modifiers::CTRL.with(Modifiers::SHIFT),
        ),
        ("echo pasted-super", vec![Modifier::Super], Modifiers::SUPER),
    ] {
        *f.clipboard.0.borrow_mut() = text.to_owned();
        wait_until("bracketed paste", || {
            f.session.pane().with_term(|t| t.paste_mode())
                == temor_pane::paste::PasteMode::Bracketed
        });
        f.session
            .key(pressed(Some(chord(&chord_mods, 'v')), KeyCode::V, mods, ""))
            .expect("the paste chord");
        press_enter(&mut f.session);
        let want = text.trim_start_matches("echo ").to_owned();
        wait_until("the pasted command's output", || {
            has_line(&f.session.screen_text(), &want)
        });
    }
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

/// The colour of the pane's top-left pixel after a paint (premultiplied RGBA, opaque ground).
fn corner_colour(session: &mut Session) -> [u8; 3] {
    let mut corner = None;
    session
        .paint(|bytes, _size| corner = Some([bytes[0], bytes[1], bytes[2]]))
        .expect("paint");
    corner.expect("the pane has a size, so it paints")
}

#[test]
fn a_theme_change_repaints_the_grid_in_the_new_ground() {
    if !super::support::tools_present() {
        return;
    }
    use ds::prelude::{Accent, Scheme};
    use temor_ui::view::theme::palette_of;
    let mut f = ready("session-theme");
    let dark = palette_of(Scheme::Dark, Accent::default());
    let light = palette_of(Scheme::Light, Accent::default());
    f.session
        .set_palette(dark)
        .expect("the dark palette applies");
    let before = corner_colour(&mut f.session);
    assert_eq!(
        before,
        [dark.background.r, dark.background.g, dark.background.b]
    );
    f.session
        .set_palette(light)
        .expect("the light palette applies");
    let after = corner_colour(&mut f.session);
    assert_eq!(
        after,
        [light.background.r, light.background.g, light.background.b]
    );
    assert_ne!(before, after, "the grid was repainted in the new ground");
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
    let _alone = super::support::heavy();
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
        f.session.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Plain
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

#[test]
fn dropped_files_are_typed_into_the_prompt_quoted_for_the_shell() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-drop");
    wait_until("bracketed paste", || {
        f.session.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Bracketed
    });
    f.session.drop_files(&[
        std::path::PathBuf::from("/tmp/a b.txt"),
        std::path::PathBuf::from("/tmp/it's.txt"),
    ]);
    wait_until("the quoted paths", || {
        f.session
            .screen_text()
            .contains(r"'/tmp/a b.txt' '/tmp/it'\''s.txt'")
    });
    // Nothing ran: a drop only types, and the person's Enter is what runs it.
    assert!(!has_line(
        &f.session.screen_text(),
        "a b.txt: command not found"
    ));

    // A name with a line break is never typed.
    f.session
        .drop_files(&[std::path::PathBuf::from("/tmp/x\ny")]);
    assert_eq!(
        f.session.take_notice().as_deref(),
        Some("That file's name can't be typed into a terminal")
    );
}

#[test]
fn hovering_a_link_shows_its_target_and_never_opens_it() {
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-link");
    f.session.pane().with_term(|t| {
        let _ = t
            .write(b"\x1b[H\x1b[2K\x1b]8;;https://example.com/a?b=1\x1b\\link\x1b]8;;\x1b\\ after");
    });
    // (column, the target shown, whether the tooltip must be drawn again)
    let table = [
        (
            1,
            Some("https://example.com/a?b=1"),
            temor_ui::session::LinkHover::Changed,
        ),
        (
            2,
            Some("https://example.com/a?b=1"),
            temor_ui::session::LinkHover::Same,
        ),
        (20, None, temor_ui::session::LinkHover::Changed),
    ];
    for (col, want, redraw) in table {
        f.session.pointer_moved(point_at(&f.session, 0, col));
        assert_eq!(
            f.session.link_under_pointer().as_deref(),
            want,
            "column {col}"
        );
        assert_eq!(
            f.session.link_tip().map(|tip| tip.target).as_deref(),
            want,
            "column {col}: the tooltip names the target"
        );
        assert_eq!(f.session.refresh_link(), redraw, "column {col}");
    }
    assert!(f.session.take_notice().is_none());
}

fn copy_event(text: &str) -> PaneEvent {
    copy_to(temor_vt::ClipboardLocation::Standard, text)
}

fn copy_to(location: temor_vt::ClipboardLocation, text: &str) -> PaneEvent {
    PaneEvent::Clipboard(temor_vt::ClipboardWrite {
        location,
        items: vec![temor_vt::ClipboardItem {
            mime: "text/plain".to_owned(),
            data: text.as_bytes().to_vec(),
        }],
        program: "tool".to_owned(),
        grant: temor_vt::ClipboardGrant::Ask,
    })
}

#[test]
fn a_program_copies_freely_here_and_asks_once_on_another_computer() {
    use temor_core::remote::Grant;
    use temor_core::shelf::PaneSite;
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-osc52-write");
    f.session.absorb(copy_event("here"));
    assert_eq!(*f.clipboard.0.borrow(), "here");

    f.session.set_site(PaneSite::Remote);
    f.session.absorb(copy_event("there"));
    assert_eq!(*f.clipboard.0.borrow(), "here", "waits for the person");
    assert_eq!(
        f.session.clipboard_ask().map(|a| (a.kind, a.program)),
        Some((temor_ui::session::ClipboardUse::Copy, "tool".to_owned()))
    );
    f.session.answer_clipboard(Grant::Allowed);
    assert_eq!(*f.clipboard.0.borrow(), "there");
    // The answer is kept: the next write goes through without a question.
    f.session.absorb(copy_event("again"));
    assert_eq!(*f.clipboard.0.borrow(), "again");
    assert!(f.session.clipboard_ask().is_none());

    f.session.set_clipboard_consent(Grant::Refused);
    f.session.absorb(copy_event("refused"));
    assert_eq!(*f.clipboard.0.borrow(), "again");
}

/// OSC 52 with `p` (or `s`) puts the text on the primary selection and leaves the clipboard
/// alone, with the same question on another computer.
#[test]
fn a_program_copies_to_the_primary_selection_when_it_names_it() {
    use temor_core::remote::Grant;
    use temor_core::shelf::PaneSite;
    use temor_vt::ClipboardLocation::{Primary, Selection};
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-osc52-primary");
    *f.clipboard.0.borrow_mut() = "kept".to_owned();
    f.session.absorb(copy_to(Primary, "picked"));
    assert_eq!(*f.clipboard.1.borrow(), "picked");
    assert_eq!(
        *f.clipboard.0.borrow(),
        "kept",
        "the clipboard is not touched"
    );
    f.session.absorb(copy_to(Selection, "selected"));
    assert_eq!(*f.clipboard.1.borrow(), "selected");

    f.session.set_site(PaneSite::Remote);
    f.session.absorb(copy_to(Primary, "there"));
    assert_eq!(*f.clipboard.1.borrow(), "selected", "waits for the person");
    f.session.answer_clipboard(Grant::Allowed);
    assert_eq!(*f.clipboard.1.borrow(), "there");
    assert_eq!(*f.clipboard.0.borrow(), "kept");
}

#[test]
fn a_program_reads_the_clipboard_only_after_the_person_allows_it() {
    use temor_core::remote::Grant;
    use temor_vt::VtEvent;
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-osc52-read");
    *f.clipboard.0.borrow_mut() = "Hello".to_owned();
    let query = b"\x1b]52;c;?\x07";
    let out = f.session.pane().with_term(|t| t.write(query));
    let read = out
        .events
        .into_iter()
        .find_map(|e| match e {
            VtEvent::ClipboardRead { read } => Some(read),
            _ => None,
        })
        .expect("the first read asks");
    f.session.absorb(PaneEvent::ClipboardRead(read));
    assert_eq!(
        f.session.clipboard_ask().map(|a| a.kind),
        Some(temor_ui::session::ClipboardUse::Read)
    );
    f.session.answer_clipboard(Grant::Allowed);
    let out = f.session.pane().with_term(|t| t.write(query));
    assert!(
        out.replies
            .iter()
            .any(|r| r.windows(8).any(|w| w == b"SGVsbG8=")),
        "the reply carries the clipboard"
    );
}

#[test]
fn the_cursor_blinks_in_the_focused_pane_and_shows_solid_after_a_key() {
    use temor_core::notify::Focus;
    if !super::support::tools_present() {
        return;
    }
    let mut f = ready("session-blink");
    let shot = |session: &mut Session| {
        let mut pixels = Vec::new();
        session
            .paint(|bytes, _| pixels = bytes.to_vec())
            .expect("paint");
        pixels
    };
    // Focus starts the blink over: the first paint draws the cursor.
    f.session.set_focus(Focus::Focused);
    let on = shot(&mut f.session);
    assert!(!f.session.blink_due(), "nothing turned yet");
    wait_until("the cursor's off half", || f.session.blink_due());
    let off = shot(&mut f.session);
    assert_ne!(on, off, "the off half leaves the cursor out");
    // A key shows it solid again at once.
    type_text(&mut f.session, "x");
    wait_until("the typed x", || f.session.screen_text().contains("$ x"));
    let typed = shot(&mut f.session);
    assert_ne!(typed, off, "the cursor is back after the key");
    // An unfocused pane's hollow cursor never blinks.
    f.session.set_focus(Focus::Unfocused);
    shot(&mut f.session);
    std::thread::sleep(Duration::from_millis(2 * temor_ui::session::BLINK_MS));
    assert!(!f.session.blink_due(), "an unfocused pane does not blink");
}

#[test]
fn a_new_font_size_reaches_an_open_pane_and_the_program_sees_the_new_grid() {
    if !super::support::tools_present() {
        return;
    }
    use temor_pane::settings::{CursorStyle, FontChoice, FontPoints, LinePercent};
    use temor_ui::view::launch::load_fonts;

    let mut f = ready("session-font-size");
    let before = f.session.fit().expect("fitted");
    let bigger = FontPoints::new(26.0).expect("in range");
    let fonts = load_fonts(&FontChoice::SystemMonospace, bigger, LinePercent::DEFAULT)
        .expect("a monospace font");
    f.session.set_fonts(fonts).expect("the pane takes the font");
    let after = f.session.fit().expect("fitted");
    assert!(
        after.cell.width.0 > before.cell.width.0 && after.cell.height.0 > before.cell.height.0,
        "{:?} -> {:?}",
        before.cell,
        after.cell
    );
    assert!(
        after.grid.cols.0 < before.grid.cols.0 && after.grid.rows.0 < before.grid.rows.0,
        "fewer, larger cells fill the same pane: {:?} -> {:?}",
        before.grid,
        after.grid
    );
    // The program is told the smaller grid.
    let want = format!("{} {}", after.grid.rows.0, after.grid.cols.0);
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
    f.session
        .set_cursor_style(CursorStyle::Bar)
        .expect("the terminal takes the cursor");
}

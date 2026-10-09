//! The pane engine on a real PTY and bash: input reaches the program, output reaches the screen,
//! a resize changes the grid and the program's size, replies go back, and the program is reaped.

use std::time::{Duration, Instant};

use temor_core::terminal::paste::{PasteMode, encode};
use temor_vt::{GridSize, KeyCode, KeyInput, Modifiers};

use super::support::{count_events, scratch, screen, spawn_bash, wait_for};

#[test]
fn a_typed_command_runs_and_its_output_reaches_the_screen() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("typing").join("home");
    let mut shell = spawn_bash(&home, 80, 24);
    wait_for(&mut shell, "the prompt", |s| s.contains("$"));
    for c in "echo temor-ok".chars() {
        let text = c.to_string();
        let key = KeyInput::press(key_code(c), Modifiers::NONE, &text);
        shell.pane.key(&key).expect("key encodes");
    }
    shell
        .pane
        .key(&KeyInput::press(KeyCode::Enter, Modifiers::NONE, ""))
        .expect("enter");
    wait_for(&mut shell, "the echoed output", |s| {
        s.lines().any(|l| l == "temor-ok")
    });
}

#[test]
fn a_resize_changes_the_grid_and_the_program_size() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("resize").join("home");
    let mut shell = spawn_bash(&home, 80, 24);
    wait_for(&mut shell, "the prompt", |s| s.contains("$"));
    shell.pane.send(b"stty size\r");
    wait_for(&mut shell, "stty size", |s| s.lines().any(|l| l == "24 80"));
    shell
        .pane
        .resize(GridSize::new(100, 30), temor_vt::CellPx::new(8, 16))
        .expect("resize");
    assert_eq!(shell.pane.with_term(|t| t.size()), GridSize::new(100, 30));
    // The program learns its size on the I/O thread, a moment after `resize` returns; ask again
    // until its answer shows the new size.
    let start = Instant::now();
    loop {
        shell.pane.send(b"stty size\r");
        std::thread::sleep(Duration::from_millis(200));
        if screen(&shell.pane).lines().any(|l| l == "30 100") {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "the program never saw 30 100"
        );
    }
}

#[test]
fn bracketed_paste_is_framed_when_the_program_asks_for_it() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("paste").join("home");
    let mut shell = spawn_bash(&home, 80, 24);
    wait_for(&mut shell, "the prompt", |s| s.contains("$"));
    // bash turns bracketed paste on at its prompt (readline's `\e[?2004h`).
    let start = Instant::now();
    while shell.pane.with_term(|t| t.paste_mode()) != PasteMode::Bracketed {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "bracketed paste never turned on"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let frame = encode("echo pasted", PasteMode::Bracketed);
    shell.pane.send(&frame);
    shell.pane.send(b"\r");
    wait_for(&mut shell, "the pasted command output", |s| {
        s.lines().any(|l| l == "pasted")
    });
}

#[test]
fn the_program_is_reaped_when_the_pane_is_dropped() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("reap").join("home");
    let shell = spawn_bash(&home, 80, 24);
    let pid_file = home.join("pid");
    shell
        .pane
        .send(format!("echo $$ > {}\r", pid_file.display()).as_bytes());
    let start = Instant::now();
    while !pid_file.exists() && start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .expect("pid written")
        .trim()
        .parse()
        .expect("pid is a number");
    drop(shell);
    // The I/O thread hangs bash up and kills it after the grace period.
    let alive = |pid: i32| std::path::Path::new(&format!("/proc/{pid}")).exists();
    let start = Instant::now();
    while alive(pid) && start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !alive(pid),
        "bash {pid} still running after its pane was dropped"
    );
}

#[test]
fn the_screen_reads_back_as_text() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("screen").join("home");
    let mut shell = spawn_bash(&home, 40, 6);
    wait_for(&mut shell, "the prompt", |s| s.contains("$"));
    assert!(screen(&shell.pane).contains("$"));
}

fn key_code(c: char) -> KeyCode {
    match c.to_ascii_lowercase() {
        'a' => KeyCode::A,
        'b' => KeyCode::B,
        'c' => KeyCode::C,
        'd' => KeyCode::D,
        'e' => KeyCode::E,
        'h' => KeyCode::H,
        'i' => KeyCode::I,
        'k' => KeyCode::K,
        'm' => KeyCode::M,
        'n' => KeyCode::N,
        'o' => KeyCode::O,
        'p' => KeyCode::P,
        'r' => KeyCode::R,
        's' => KeyCode::S,
        't' => KeyCode::T,
        'x' => KeyCode::X,
        'y' => KeyCode::Y,
        '-' => KeyCode::Minus,
        _ => KeyCode::Space,
    }
}

#[test]
fn a_flood_of_output_costs_a_bounded_number_of_events() {
    if !super::support::tools_present() {
        return;
    }
    let home = scratch("flood").join("home");
    let mut shell = spawn_bash(&home, 80, 24);
    wait_for(&mut shell, "the prompt", |s| s.contains("$"));
    count_events(&mut shell.events);
    shell.pane.send(b"seq 1 200000\r");
    // Count every event while the output streams; `wait_for` would consume them.
    let start = Instant::now();
    let mut events = 0;
    loop {
        events += count_events(&mut shell.events);
        if screen(&shell.pane).lines().any(|l| l == "200000") {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "the flood never finished"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    events += count_events(&mut shell.events);
    // Nothing repaints in this test, so at most one repaint request is in flight: two hundred
    // thousand lines of output are a handful of events, not one per read.
    assert!(events <= 5, "{events} events for a flood");
}

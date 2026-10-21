//! Text the keyboard and the shell must carry whole (a break-it pass for macOS, where emoji and
//! CJK are typed through the input method): wide characters committed by the IME and echoed by
//! the shell, a composition that is abandoned typing nothing, and tab churn that leaves the pane
//! answering. The harness's plain `Input::key` carries no text for a non-ASCII character, so
//! the IME is the way in.

use ds_harness::Input;
use temor_ui::view::probe::Probe;

use super::drive::{chord, started, type_line, type_text, wait_until};

#[test]
fn emoji_and_cjk_committed_by_the_ime_come_back_whole() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("wide-text-echo", &probe);
    type_text(&mut h, "echo [");
    for text in ["日本語", "😀", "é"] {
        h.send(Input::ime_start());
        h.send(Input::ime_commit(text));
        h.send(Input::ime_end());
    }
    type_line(&mut h, "]");
    wait_until(&mut h, &probe, "the echoed text", |s| {
        s.lines().any(|l| l.trim_end() == "[日本語😀é]")
    });
}

#[test]
fn a_composition_given_up_types_nothing() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("wide-text-abandoned", &probe);
    type_text(&mut h, "echo [");
    h.send(Input::ime_start());
    h.send(Input::ime_update("にほん", 3));
    h.send(Input::ime_end());
    type_line(&mut h, "]");
    wait_until(&mut h, &probe, "the echoed brackets", |s| {
        s.lines().any(|l| l.trim_end() == "[]")
    });
}

#[test]
fn tabs_opened_in_a_burst_leave_a_pane_that_still_answers() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("wide-text-burst", &probe);
    for _ in 0..5 {
        chord(&mut h, 'n');
    }
    wait_until(&mut h, &probe, "the newest tab's prompt", |s| {
        s.contains('$')
    });
    type_line(&mut h, "echo burst$((40+2))");
    wait_until(&mut h, &probe, "burst42", |s| s.contains("burst42"));
}

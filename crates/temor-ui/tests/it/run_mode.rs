//! `temor -e`: a first launch asked to run a program opens with a tab that runs it in place of
//! the shell, in the asker's folder with the asker's environment. The tab stays when the program
//! ends, says so by its name, and Restart runs it again.

use std::ffi::OsString;
use std::time::{Duration, Instant};

use ds_harness::{Harness, Input};
use temor_desktop::{CLOSED_WHILE_RUNNING, NOT_FOUND, RunEnd, RunEnds, RunTicket};
use temor_shell::ChildEnv;
use temor_ui::hub::Run;
use temor_ui::view::probe::Probe;

use super::support::{scratch, tools_present};
use super::window::{chord, harness_running, harness_running_to, rows, wait_until};

fn wait_for_html(h: &mut Harness, what: &str, text: &str) {
    let start = Instant::now();
    while !h.html().contains(text) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

#[test]
fn a_run_tab_runs_the_program_where_it_was_asked_and_restart_runs_it_again() {
    if !tools_present() {
        return;
    }
    let work = scratch("run-mode-work");
    std::fs::create_dir_all(&work).expect("work folder");
    let probe = Probe::new();
    let script =
        "echo x >> runs; echo \"$GREETING from $(basename \"$PWD\"), run $(wc -l < runs)\"";
    let mut env = ChildEnv::new();
    env.set("GREETING", "hello");
    let run = Run {
        argv: vec![
            OsString::from("bash"),
            OsString::from("-c"),
            OsString::from(script),
        ],
        cwd: Some(work.clone()),
        env,
        ticket: None,
    };
    let mut h = harness_running("run-mode", &probe, run);
    wait_until(&mut h, &probe, "the program's line", |s| {
        s.contains("hello from run-mode-work, run 1")
    });
    wait_for_html(&mut h, "the ended banner", "bash has ended.");
    assert!(!h.html().contains("The shell has ended."));
    // One tab: the run's, not an empty shell beside it.
    assert_eq!(rows(&h), 1, "{}", h.html());

    let restart = h
        .centre(".ds-inline-banner .ds-button")
        .unwrap_or_else(|| panic!("no Restart button:\n{}", h.html()));
    h.send(Input::click(restart));
    wait_until(&mut h, &probe, "the program run again", |s| {
        s.contains("hello from run-mode-work, run 2")
    });
}

#[test]
fn a_program_that_cannot_be_found_is_said_and_the_window_still_opens_a_shell() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let run = Run {
        argv: vec![OsString::from("no-such-program-for-temor")],
        cwd: None,
        env: ChildEnv::new(),
        ticket: None,
    };
    let mut h = harness_running("run-mode-missing", &probe, run);
    wait_for_html(
        &mut h,
        "the notice",
        "temor couldn't find the program “no-such-program-for-temor”.",
    );
    wait_until(&mut h, &probe, "a shell's prompt", |s| s.contains('$'));
}

#[test]
fn closing_a_run_tab_tells_the_waiting_launch_how_its_program_ended() {
    if !tools_present() {
        return;
    }
    let (ends, mut heard) = tokio::sync::mpsc::unbounded_channel();
    let ticket = RunTicket(7);
    let run = |script: &str| Run {
        argv: vec![
            OsString::from("bash"),
            OsString::from("-c"),
            OsString::from(script),
        ],
        cwd: None,
        env: ChildEnv::new(),
        ticket: Some(ticket),
    };
    let probe = Probe::new();
    let mut h = harness_running_to(
        "run-mode-ends",
        &probe,
        run("echo bye; exit 3"),
        RunEnds::new(ends),
    );
    wait_for_html(&mut h, "the ended banner", "bash has ended.");
    // Ended is not closed: the launch still waits while the tab stays.
    assert!(heard.try_recv().is_err());
    chord(&mut h, 'w');
    let end = heard.try_recv().expect("the launch hears the tab closed");
    assert_eq!(end, RunEnd { ticket, status: 3 });
}

#[test]
fn a_run_tab_closed_while_its_program_runs_ends_as_hung_up() {
    if !tools_present() {
        return;
    }
    let (ends, mut heard) = tokio::sync::mpsc::unbounded_channel();
    let ticket = RunTicket(8);
    let run = Run {
        argv: vec![
            OsString::from("bash"),
            OsString::from("-c"),
            OsString::from("echo waiting; sleep 600"),
        ],
        cwd: None,
        env: ChildEnv::new(),
        ticket: Some(ticket),
    };
    let probe = Probe::new();
    let mut h = harness_running_to("run-mode-hangup", &probe, run, RunEnds::new(ends));
    wait_until(&mut h, &probe, "the program", |s| s.contains("waiting"));
    chord(&mut h, 'w');
    let end = heard.try_recv().expect("the launch hears the tab closed");
    assert_eq!(
        end,
        RunEnd {
            ticket,
            status: CLOSED_WHILE_RUNNING
        }
    );
}

#[test]
fn a_program_that_cannot_be_found_ends_its_launch_at_once() {
    if !tools_present() {
        return;
    }
    let (ends, mut heard) = tokio::sync::mpsc::unbounded_channel();
    let run = Run {
        argv: vec![OsString::from("no-such-program-for-temor")],
        cwd: None,
        env: ChildEnv::new(),
        ticket: Some(RunTicket(9)),
    };
    let probe = Probe::new();
    let mut h = harness_running_to("run-mode-gone", &probe, run, RunEnds::new(ends));
    wait_for_html(&mut h, "the notice", "couldn't find the program");
    assert_eq!(
        heard.try_recv().expect("the launch is told"),
        RunEnd {
            ticket: RunTicket(9),
            status: NOT_FOUND
        }
    );
}

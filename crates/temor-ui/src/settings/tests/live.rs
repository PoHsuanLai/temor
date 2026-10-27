//! What a change of settings means for open panes.

use temor_panes::pane::settings::{
    CursorBlinking, CursorStyle, FontChoice, FontPoints, LinePercent, NotifyAfter, TerminalSettings,
};

use crate::settings::{LiveChange, live_changes};

#[test]
fn the_look_and_the_notice_threshold_reach_open_panes() {
    let old = TerminalSettings::default();
    let size = FontPoints::new(20.0).expect("in range");
    // (what changes, what open panes are told)
    let rows: Vec<(TerminalSettings, LiveChange)> = vec![
        (old.clone(), LiveChange::default()),
        (
            TerminalSettings {
                font_size: size,
                ..old.clone()
            },
            LiveChange {
                font: Some((FontChoice::SystemMonospace, size, LinePercent::DEFAULT)),
                ..LiveChange::default()
            },
        ),
        (
            TerminalSettings {
                cursor: CursorStyle::Bar,
                cursor_blink: CursorBlinking::Off,
                ..old.clone()
            },
            LiveChange {
                cursor: Some(CursorStyle::Bar),
                blink: Some(CursorBlinking::Off),
                ..LiveChange::default()
            },
        ),
        (
            TerminalSettings {
                notify_after: NotifyAfter::Seconds(30),
                ..old.clone()
            },
            LiveChange {
                notify: Some(NotifyAfter::Seconds(30)),
                ..LiveChange::default()
            },
        ),
        // These are read when a pane starts.
        (
            TerminalSettings {
                shell: temor_panes::pane::settings::ShellChoice::Program("fish".to_owned()),
                offer_add: vec!["ssh".to_owned()],
                ..old.clone()
            },
            LiveChange::default(),
        ),
    ];
    for (new, want) in rows {
        let got = live_changes(&old, &new);
        assert_eq!(got, want);
        assert_eq!(got.is_none(), want == LiveChange::default());
    }
}

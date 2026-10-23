//! Ctrl+Shift+Up and Down, driven by direct calls on a real shell.

use chordkit::{Key, KeyInput as Pressed, Modifier, Modifiers as ChordMods, NamedKey};
use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_ui::input::Stroke;
use temor_ui::session::Session;
use temor_vt::{KeyCode, KeyInput, Modifiers};

use super::super::support::tools_present;
use super::support::{marks_of, pump, run_line, shell_path, start};

#[test]
fn control_shift_up_and_down_pick_blocks_and_bring_them_into_view() {
    if !tools_present() {
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
    let prompt_top = |s: &Session| marks_of(s).iter().find(|m| m.row == 0).map(|m| m.id);
    let up = Stroke {
        input: Some(Pressed::new(
            [Modifier::Ctrl, Modifier::Shift]
                .into_iter()
                .collect::<ChordMods>(),
            Key::Named(NamedKey::Up),
        )),
        key: KeyInput::press(KeyCode::ArrowUp, Modifiers::CTRL.with(Modifiers::SHIFT), ""),
    };
    let down = Stroke {
        input: Some(Pressed::new(
            [Modifier::Ctrl, Modifier::Shift]
                .into_iter()
                .collect::<ChordMods>(),
            Key::Named(NamedKey::Down),
        )),
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
    assert_eq!(
        live.session.picked_blocks(),
        [ids[2]],
        "and its block picked"
    );
    live.session.key(up).expect("up again");
    assert_eq!(prompt_top(&live.session), Some(ids[1]));
    live.session.key(down).expect("down");
    assert_eq!(prompt_top(&live.session), Some(ids[2]));
}

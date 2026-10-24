//! Ctrl+Shift+Up and Down, driven by direct calls on a real shell.

use chordkit::{Key, KeyInput as Pressed, Modifier, Modifiers as ChordMods, NamedKey};
use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_ui::input::Stroke;
use temor_vt::{KeyCode, KeyInput, Modifiers};

use super::super::support::tools_present;
use super::support::{pump, run_line, shell_path, start};

#[test]
fn control_shift_up_and_down_pick_blocks() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-jump", &shell);
    for round in 1..=3 {
        run_line(&live, "seq 1 30");
        // The prompt after the block, so the next line is typed at it.
        pump(&mut live, "the prompt after the block", |s| {
            s.prompts_seen() > round
                && s.block_outcomes()
                    .last()
                    .is_some_and(|b| b.1 != Outcome::Running)
        });
    }
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
    // The list scrolls the picked card into view (BlockPane's `reveal`); the terminal's own
    // viewport stays at its bottom.
    live.session.key(up.clone()).expect("up");
    assert_eq!(live.session.picked_blocks(), [ids[2]], "the newest block");
    live.session.key(up).expect("up again");
    assert_eq!(live.session.picked_blocks(), [ids[1]], "the one before it");
    live.session.key(down).expect("down");
    assert_eq!(live.session.picked_blocks(), [ids[2]], "and back");
}

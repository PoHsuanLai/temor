//! The gutter, the prompt jumps and the block actions, driven by direct calls on a real shell.

use temor_core::block::Outcome;
use temor_core::terminal::action::Action;
use temor_core::terminal::chord::Chord;
use temor_core::vocab::ids::BlockId;
use temor_ui::input::Stroke;
use temor_ui::session::Session;
use temor_vt::{KeyCode, KeyInput, Modifiers};

use super::super::support::tools_present;
use super::support::{pump, run_line, shell_path, start};

#[test]
fn a_gutter_press_selects_the_block_output_and_copies_it() {
    if !tools_present() {
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

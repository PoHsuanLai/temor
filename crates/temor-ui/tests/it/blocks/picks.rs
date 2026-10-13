//! Picking whole blocks, as Warp does, driven by direct calls on a real shell: a click picks a
//! block, Shift+click a run, Ctrl+click one in or out; Ctrl+Shift+Up picks the newest block, the
//! arrows move and grow the pick, Escape
//! and typing let go of it; a drag still selects text; and copy takes the picked blocks whole.

use std::time::Duration;

use chordkit::{Key, KeyInput as Pressed, Modifiers as ChordMods, NamedKey};
use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_pane::keys::Action;
use temor_ui::input::Stroke;
use temor_vt::{KeyCode, KeyInput, Modifiers, MouseButton, SurfacePoint};

use super::super::support::tools_present;
use super::support::{Live, pump, run_line, shell_path, start};

/// The point in the middle of cell (`col`, viewport `row`).
fn cell(live: &Live, col: u16, row: u32) -> SurfacePoint {
    let fit = live.session.fit().expect("fitted");
    let (x, y) = fit.cell_origin(temor_render::CellPos::new(
        col,
        u16::try_from(row).expect("row"),
    ));
    SurfacePoint {
        x: x.0 as i32 + 2,
        y: y.0 as i32 + 2,
    }
}

/// The header row of block `id` in the viewport.
fn header_of(live: &Live, id: BlockId) -> u32 {
    live.session
        .block_marks()
        .into_iter()
        .find(|m| m.id == id)
        .expect("the block is on screen")
        .header
}

/// A click that stays in its cell; each one far enough from the last not to make a double click.
fn click(live: &mut Live, at: SurfacePoint, mods: Modifiers, clock: &mut u64) {
    *clock += 5;
    let now = Duration::from_secs(*clock);
    live.session
        .press(at, MouseButton::Left, mods, now)
        .expect("press");
    live.session
        .release(at, MouseButton::Left, mods)
        .expect("release");
}

fn stroke(code: KeyCode, named: Option<NamedKey>, mods: Modifiers, text: &str) -> Stroke {
    Stroke {
        input: named.map(|n| Pressed::new(ChordMods::default(), Key::Named(n))),
        key: KeyInput::press(code, mods, text),
    }
}

fn copied(live: &mut Live) -> String {
    *live.clipboard.0.borrow_mut() = String::new();
    live.session.run(Action::Copy).expect("copy");
    live.clipboard.0.borrow().clone()
}

/// A bash with three finished blocks, `echo a`, `echo b` and `echo c`, oldest first.
fn three_blocks(name: &str) -> Option<(Live, [BlockId; 3])> {
    if !tools_present() {
        return None;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start(name, &shell);
    for word in ["a", "b", "c"] {
        let before = live.session.block_outcomes().len();
        run_line(&live, &format!("echo {word}"));
        pump(&mut live, "a finished block", |s| {
            s.block_outcomes().len() > before
                && s.block_outcomes()
                    .last()
                    .is_some_and(|b| b.1 != Outcome::Running)
        });
    }
    let ids: Vec<BlockId> = live
        .session
        .block_outcomes()
        .into_iter()
        .map(|b| b.0)
        .collect();
    Some((live, [ids[0], ids[1], ids[2]]))
}

#[test]
fn a_click_picks_a_block_and_copy_takes_its_command_and_output() {
    let Some((mut live, [a, b, c])) = three_blocks("picks-click") else {
        return;
    };
    let mut clock = 0;
    let at_b = cell(&live, 3, header_of(&live, b) + 2);
    click(&mut live, at_b, Modifiers::NONE, &mut clock);
    assert_eq!(live.session.picked_blocks(), [b], "the clicked block");
    assert_eq!(copied(&mut live), "echo b\nb");
    let at_c = cell(&live, 3, header_of(&live, c) + 1);
    click(&mut live, at_c, Modifiers::SHIFT, &mut clock);
    assert_eq!(live.session.picked_blocks(), [b, c], "Shift: the run");
    assert_eq!(copied(&mut live), "echo b\nb\n\necho c\nc");
    click(&mut live, at_b, Modifiers::CTRL, &mut clock);
    assert_eq!(live.session.picked_blocks(), [c], "Ctrl: out again");
    let at_a = cell(&live, 3, header_of(&live, a));
    click(&mut live, at_a, Modifiers::CTRL, &mut clock);
    assert_eq!(live.session.picked_blocks(), [a, c], "Ctrl: in");
    // The live prompt is no block: a click there lets go.
    let rows = live.session.screen_text().lines().count();
    let prompt = cell(&live, 1, u32::try_from(rows).expect("rows") - 1);
    click(&mut live, prompt, Modifiers::NONE, &mut clock);
    assert!(
        live.session.picked_blocks().is_empty(),
        "the prompt lets go"
    );
}

#[test]
fn the_arrows_move_and_grow_the_pick_and_escape_or_typing_lets_go() {
    let Some((mut live, [a, b, c])) = three_blocks("picks-keys") else {
        return;
    };
    let mut clock = 0;
    let up = stroke(KeyCode::ArrowUp, Some(NamedKey::Up), Modifiers::NONE, "");
    let down = stroke(
        KeyCode::ArrowDown,
        Some(NamedKey::Down),
        Modifiers::NONE,
        "",
    );
    let shift_up = stroke(KeyCode::ArrowUp, Some(NamedKey::Up), Modifiers::SHIFT, "");
    let escape = stroke(KeyCode::Escape, Some(NamedKey::Escape), Modifiers::NONE, "");
    // Ctrl+Shift+Up with nothing picked: the newest block, though every block is in view.
    live.session.run(Action::PreviousPrompt).expect("previous");
    assert_eq!(live.session.picked_blocks(), [c], "the newest block");
    live.session.key(up.clone()).expect("up");
    assert_eq!(live.session.picked_blocks(), [b]);
    live.session.key(shift_up).expect("shift up");
    assert_eq!(live.session.picked_blocks(), [a, b], "grown upwards");
    live.session.key(escape).expect("escape");
    assert!(live.session.picked_blocks().is_empty(), "Escape lets go");
    let at_c = cell(&live, 3, header_of(&live, c) + 2);
    click(&mut live, at_c, Modifiers::NONE, &mut clock);
    live.session.key(down).expect("down");
    assert!(
        live.session.picked_blocks().is_empty(),
        "past the newest block: back at the prompt"
    );
    click(&mut live, at_c, Modifiers::NONE, &mut clock);
    live.session.key(up).expect("up");
    let typed = stroke(KeyCode::X, None, Modifiers::NONE, "x");
    live.session.key(typed).expect("x");
    assert!(live.session.picked_blocks().is_empty(), "typing lets go");
}

#[test]
fn a_drag_selects_text_and_lets_go_of_the_pick() {
    let Some((mut live, [_, b, _])) = three_blocks("picks-drag") else {
        return;
    };
    let mut clock = 0;
    let row = header_of(&live, b) + 1;
    let on_command = cell(&live, 2, row);
    click(&mut live, on_command, Modifiers::NONE, &mut clock);
    assert_eq!(live.session.picked_blocks(), [b]);
    let (from, to) = (cell(&live, 0, row), cell(&live, 3, row));
    live.session
        .press(
            from,
            MouseButton::Left,
            Modifiers::NONE,
            Duration::from_secs(99),
        )
        .expect("press");
    live.session.drag_to(to).expect("drag");
    live.session
        .release(to, MouseButton::Left, Modifiers::NONE)
        .expect("release");
    assert!(live.session.picked_blocks().is_empty(), "the drag let go");
    assert_eq!(copied(&mut live), "echo", "the text the drag covered");
}

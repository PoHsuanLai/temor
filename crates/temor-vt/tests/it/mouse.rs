//! SGR mouse reporting (modes 1000 + 1006): press and release at a cell.

use temor_vt::{
    Modifiers, MouseAction, MouseButton, MouseEncoder, MouseInput, MouseSurface, Px, SurfacePoint,
};

use crate::support::term;

fn surface() -> MouseSurface {
    MouseSurface {
        width: Px(800),
        height: Px(480),
        cell_width: Px(10),
        cell_height: Px(20),
        padding: [Px(0); 4],
    }
}

fn input(action: MouseAction, button: Option<MouseButton>, x: i32, y: i32) -> MouseInput {
    MouseInput {
        action,
        button,
        mods: Modifiers::NONE,
        at: SurfacePoint { x, y },
    }
}

#[test]
fn sgr_mouse_reports_press_and_release() {
    let (mut t, _rx) = term(80, 24);
    let mut enc = MouseEncoder::new().expect("encoder");
    enc.set_surface(surface());
    enc.sync(&t);
    let press = input(MouseAction::Press, Some(MouseButton::Left), 55, 65);
    assert_eq!(
        enc.encode(&press).expect("encode"),
        b"",
        "nothing before the program asks"
    );

    t.write(b"\x1b[?1000h\x1b[?1006h");
    enc.sync(&t);
    assert_eq!(enc.encode(&press).expect("encode"), b"\x1b[<0;6;4M");
    let release = input(MouseAction::Release, Some(MouseButton::Left), 55, 65);
    assert_eq!(enc.encode(&release).expect("encode"), b"\x1b[<0;6;4m");
    let wheel = input(MouseAction::Press, Some(MouseButton::WheelUp), 5, 5);
    assert_eq!(enc.encode(&wheel).expect("encode"), b"\x1b[<64;1;1M");
}

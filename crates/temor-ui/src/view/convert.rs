//! Dioxus events and their values as the terminal reads them: device pixels, lines of a wheel
//! turn, and the modifiers and buttons.

use dioxus::html::Modifiers as HeldMods;
use dioxus::html::geometry::WheelDelta;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::host::signals::HostSignals;
use temor_render::Scale;
use temor_vt::{Modifiers, MouseButton as VtButton, SurfacePoint};

use crate::session::WheelDirection;

/// The device scale the host reports, or 1x before it does.
pub fn scale_of(signals: Option<&HostSignals>) -> Scale {
    signals.map_or(Scale::new(1.0), |s| {
        Scale::new(s.scale.read().as_f64() as f32)
    })
}

/// A point in logical pixels, relative to the pane, in device pixels.
pub fn device_point(x: f64, y: f64, scale: Scale) -> SurfacePoint {
    let f = f64::from(scale.factor());
    SurfacePoint {
        x: (x * f).round() as i32,
        y: (y * f).round() as i32,
    }
}

/// Which way a wheel turn goes, and how many lines it scrolls: a line delta is three lines per
/// unit, a pixel delta one line per 20 logical pixels, and at least one line for any movement.
/// No movement is no lines.
///
/// Blitz hands the window's delta on as the system gave it (winit's): a positive `y` moves the
/// content down, so the view goes up into the scrollback, and the system has already turned the
/// sign for "natural" scrolling. This is not the DOM's `deltaY`, whose positive is down.
pub fn wheel_lines(delta: WheelDelta) -> (WheelDirection, u32) {
    let dy = match delta {
        WheelDelta::Pixels(v) => v.y / 20.0,
        WheelDelta::Lines(v) => v.y * 3.0,
        WheelDelta::Pages(v) => v.y * 24.0,
    };
    let direction = match dy > 0.0 {
        true => WheelDirection::Up,
        false => WheelDirection::Down,
    };
    let lines = match dy == 0.0 {
        true => 0,
        false => dy.abs().round().max(1.0) as u32,
    };
    (direction, lines)
}

pub fn vt_mods(held: HeldMods) -> Modifiers {
    crate::input::vt_mods(held)
}

/// The primary, secondary and middle buttons; others are not reported.
pub fn vt_button(button: MouseButton) -> Option<VtButton> {
    Some(match button {
        MouseButton::Primary => VtButton::Left,
        MouseButton::Secondary => VtButton::Right,
        MouseButton::Auxiliary => VtButton::Middle,
        _ => return None,
    })
}

#[cfg(test)]
mod wheel_tests {
    use dioxus::html::geometry::{WheelDelta, euclid::Vector3D};

    use super::{WheelDirection, wheel_lines};

    #[test]
    fn a_positive_delta_scrolls_up_and_a_negative_one_down_as_the_system_gives_it() {
        // (delta, direction, lines)
        let cases = [
            (
                WheelDelta::Pixels(Vector3D::new(0.0, 40.0, 0.0)),
                WheelDirection::Up,
                2,
            ),
            (
                WheelDelta::Pixels(Vector3D::new(0.0, -40.0, 0.0)),
                WheelDirection::Down,
                2,
            ),
            (
                WheelDelta::Lines(Vector3D::new(0.0, 1.0, 0.0)),
                WheelDirection::Up,
                3,
            ),
            (
                WheelDelta::Lines(Vector3D::new(0.0, -1.0, 0.0)),
                WheelDirection::Down,
                3,
            ),
            (
                WheelDelta::Pixels(Vector3D::new(0.0, 1.0, 0.0)),
                WheelDirection::Up,
                1,
            ),
        ];
        for (delta, direction, lines) in cases {
            assert_eq!(wheel_lines(delta), (direction, lines), "{delta:?}");
        }
        assert_eq!(
            wheel_lines(WheelDelta::Pixels(Vector3D::new(0.0, 0.0, 0.0))).1,
            0
        );
    }
}

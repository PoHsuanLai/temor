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
pub fn wheel_lines(delta: WheelDelta) -> (WheelDirection, u32) {
    let dy = match delta {
        WheelDelta::Pixels(v) => v.y / 20.0,
        WheelDelta::Lines(v) => v.y * 3.0,
        WheelDelta::Pages(v) => v.y * 24.0,
    };
    let direction = match dy < 0.0 {
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

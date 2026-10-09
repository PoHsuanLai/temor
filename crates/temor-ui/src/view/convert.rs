//! Dioxus events and their values as the terminal reads them: device pixels, lines of a wheel
//! turn, and the modifiers and buttons.

use dioxus::html::Modifiers as HeldMods;
use dioxus::html::geometry::WheelDelta;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::host::signals::HostSignals;
use temor_render::Scale;
use temor_vt::{Modifiers, MouseButton as VtButton, SurfacePoint};

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

/// Whether a wheel turn goes up, and how many lines it scrolls: a line delta is three lines per
/// unit, a pixel delta one line per 20 logical pixels, and at least one line either way.
pub fn wheel_lines(delta: WheelDelta) -> (bool, u32) {
    let dy = match delta {
        WheelDelta::Pixels(v) => v.y / 20.0,
        WheelDelta::Lines(v) => v.y * 3.0,
        WheelDelta::Pages(v) => v.y * 24.0,
    };
    (dy < 0.0, dy.abs().round().max(1.0) as u32)
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

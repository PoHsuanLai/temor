//! Mouse events for programs that track the mouse.

use super::Modifiers;
use crate::Px;

/// Press, release or motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

/// Which button. Wheel steps are `WheelUp`/`WheelDown` presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
    Back,
    Forward,
}

/// The wire format reports use. `Auto` follows the modes the program set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MouseFormat {
    #[default]
    Auto,
    X10,
    Utf8,
    Sgr,
    Urxvt,
    SgrPixels,
}

/// A position on the terminal's surface in pixels, from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfacePoint {
    pub x: i32,
    pub y: i32,
}

/// The surface geometry the encoder maps pixels to cells with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseSurface {
    pub width: Px,
    pub height: Px,
    pub cell_width: Px,
    pub cell_height: Px,
    /// Padding inside the surface before the grid starts: top, right, bottom, left.
    pub padding: [Px; 4],
}

/// One mouse event for the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseInput {
    pub action: MouseAction,
    /// `None` for motion with no button held.
    pub button: Option<MouseButton>,
    pub mods: Modifiers,
    pub at: SurfacePoint,
}

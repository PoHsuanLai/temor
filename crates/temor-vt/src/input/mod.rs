//! temor's own key and mouse input types; the encoders turn them into bytes for the program.
//! Window-system types (winit and the like) never cross this boundary.

mod key;
mod mouse;

pub use key::{KeyAction, KeyCode, KeyInput, KittyKeyFlags, Modifiers, OptionAsAlt};
pub use mouse::{MouseAction, MouseButton, MouseFormat, MouseInput, MouseSurface, SurfacePoint};

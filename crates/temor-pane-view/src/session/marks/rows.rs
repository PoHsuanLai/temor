//! Which block a point in the pane belongs to (PLAN §9 blocks).

use super::super::Session;

impl Session {
    /// Device pixels to a logical pixel on this pane's screen.
    pub fn scale_factor(&self) -> f32 {
        self.scale.factor()
    }
}

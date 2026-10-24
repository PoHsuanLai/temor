//! Which block a point in the pane belongs to (PLAN §9 blocks).

use temor_core::vocab::ids::BlockId;
use temor_vt::SurfacePoint;

use super::super::Session;

impl Session {
    /// The block under a point in the pane (device pixels), anywhere in the pane. `None` on a
    /// full-screen program's screen.
    pub fn block_at_point(&self, at: SurfacePoint) -> Option<BlockId> {
        let hit = self.point_hit(at)?;
        self.hit_block(&hit)
    }

    /// Device pixels to a logical pixel on this pane's screen.
    pub fn scale_factor(&self) -> f32 {
        self.scale.factor()
    }
}

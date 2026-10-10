//! The hook that captures a frame when a render hold begins (PLAN §4b).

use super::terminal::Terminal;

/// Called when the program starts a synchronized update that outlives the write carrying it
/// (`CSI ? 2026 h` with no matching end later in the same write). The terminal holds exactly the
/// frame the program wants left on screen: capture it now, because the next bytes change it.
/// Runs inside `write`, on the writer's thread, with the terminal borrowed; `Send` because the
/// terminal moves between threads.
pub trait HoldHook: Send {
    /// A hold just began.
    fn held(&mut self, term: &mut Terminal);
}

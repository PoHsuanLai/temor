//! The status radar (§8.1): the sources of a pane's agent status, their precedence and timing
//! rules. It feeds the lower-level `presence_step`.

mod hint;
mod model;
mod records;
mod step;
#[cfg(test)]
mod tests;

pub use hint::title_hint;
pub use model::{Millis, ProcessFact, ProgramFact, ProgramKind, RadarIn, RadarParams};
pub use step::{RadarState, radar_step};

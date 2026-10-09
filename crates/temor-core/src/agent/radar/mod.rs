//! The status radar (§8.1): the sources of a pane's agent status, their precedence and timing
//! rules, and the orb the sidebar shows. It feeds the lower-level `presence_step`.

mod hint;
mod model;
mod records;
mod step;
#[cfg(test)]
mod tests;

pub use hint::{TitleHint, title_hint};
pub use model::{
    Millis, Orb, ProcessFact, ProgramFact, ProgramKind, RadarIn, RadarOut, RadarParams, orb,
};
pub use step::{RadarState, radar_step};

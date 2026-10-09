//! temor-render's integration tests: golden images of real terminal frames, dirty-row
//! composition, and an ignored frame-time benchmark. They need the real terminal (feature
//! `ghostty`, the default).
#![cfg(feature = "ghostty")]

mod bench;
mod dirty;
mod golden;
#[cfg(feature = "gpu")]
mod gpu;
mod palette;
mod support;

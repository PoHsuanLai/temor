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

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

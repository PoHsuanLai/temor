//! temor-pane's engine tests: the pane on a real PTY and bash (scratch HOME), driven by direct
//! calls. Where bash, setsid or /proc is missing the tests skip (a failure under
//! TEMOR_REQUIRE_TOOLS=1).

#[cfg(feature = "ghostty")]
mod clipboard;
#[cfg(feature = "ghostty")]
mod engine;
#[cfg(feature = "ghostty")]
mod holds;
#[cfg(feature = "ghostty")]
mod search;
#[cfg(feature = "ghostty")]
mod search_bench;
#[cfg(feature = "ghostty")]
mod support;
#[cfg(feature = "ghostty")]
mod transfers;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

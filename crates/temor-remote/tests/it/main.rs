//! temor-remote's tests: an in-process daemon and client over pipes, real terminals running the
//! test shells, a scratch HOME and scratch folders (never the real ones). Where bash is missing
//! the terminal tests skip, and fail under TEMOR_REQUIRE_TOOLS=1.

#[cfg(feature = "ghostty")]
mod facts;
#[cfg(feature = "ghostty")]
mod files;
#[cfg(feature = "ghostty")]
mod fixtures;
#[cfg(feature = "ghostty")]
mod handshake;
#[cfg(feature = "ghostty")]
mod live;
#[cfg(feature = "ghostty")]
mod pipes;
#[cfg(feature = "ghostty")]
mod ports;
#[cfg(feature = "ghostty")]
mod revision;
#[cfg(feature = "ghostty")]
mod sessions;
#[cfg(feature = "ghostty")]
mod slow_link;
#[cfg(feature = "ghostty")]
mod support;
#[cfg(feature = "ghostty")]
mod sweep;
#[cfg(feature = "ghostty")]
mod transfer;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

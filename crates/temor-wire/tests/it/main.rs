//! The crate's integration tests: round trips, the decoder fed byte by byte, garbage input,
//! version agreement and the reconnect machine, all over generated values.

mod decoder;
mod future;
mod garbage;
mod golden;
mod link;
mod negotiate;
mod roundtrip;
mod support;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

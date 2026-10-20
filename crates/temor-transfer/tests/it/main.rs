//! temor-transfer's tests: parsers, the receive machine, traversal refusal, the shelf and fuzzing.
//! Filesystem tests use temporary directories only.

mod fuzz;
#[cfg(target_os = "linux")]
mod kitty_interop;
mod kitty_receive;
mod kitty_receive_limits;
mod kitty_receive_names;
mod kitty_send;
mod kitty_wire;
#[cfg(target_os = "linux")]
mod kitty_writes;
mod osc1337;
mod paths;
mod receive_support;
#[cfg(target_os = "linux")]
mod roundtrip;
mod shelf;
mod support;

#[test]
fn every_module_in_this_directory_is_declared() {
    temor_testkit::assert_modules_declared(env!("CARGO_MANIFEST_DIR"));
}

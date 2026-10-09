//! temor-transfer's tests: parsers, the receive machine, traversal refusal, the shelf and fuzzing.
//! Filesystem tests use temporary directories only.

mod fuzz;
mod kitty_receive;
mod kitty_receive_limits;
mod kitty_receive_names;
mod kitty_wire;
#[cfg(target_os = "linux")]
mod kitty_writes;
mod osc1337;
mod paths;
mod receive_support;
mod shelf;
mod support;

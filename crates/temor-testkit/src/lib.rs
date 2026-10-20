//! Test helpers every crate's tests share. L0: it names no quire, dioxus, docket, zbus or tokio
//! crate, and only a `[dev-dependencies]` section may name it.
//!
//! - [`program_or_skip`], [`optional_program`] and [`skip`] find a tool or skip the test, and
//!   fail it when `TEMOR_REQUIRE_TOOLS=1`;
//! - [`Scratch`] and [`ScratchRoot`] are scratch HOME and XDG folders, removed when dropped;
//! - [`eventually`], [`eventually_some`] and [`pid_from_file`] poll until a deadline;
//! - [`PrivateBus`] is a private session bus;
//! - [`assert_modules_declared`] is the "every test module is in `main.rs`" check.

mod bus;
mod modules;
mod program;
mod scratch;
mod wait;

pub use bus::PrivateBus;
pub use modules::assert_modules_declared;
pub use program::{
    find_program, optional_program, program_or_skip, programs_or_skip, skip, tools_required,
};
pub use scratch::{SOCKET_PATH_MAX, Scratch, ScratchRoot};
pub use wait::{SLOW, contains, eventually, eventually_some, find, pid_from_file, proc_alive};

#[cfg(test)]
mod tests;

//! temor's data, as types (PLAN §2b): what every other crate names when it speaks of a session,
//! a block, a place or a page. The store keeps these, the client hands them out, the window draws
//! them. Pure: nothing here reads a file, a clock or the network.

pub mod error;
pub mod home;
pub mod ids;
pub mod lenient;
pub mod secret;
pub mod session;
pub mod text;
pub mod url;

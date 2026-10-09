//! The crate's integration tests: round trips, the decoder fed byte by byte, garbage input,
//! version agreement and the reconnect machine, all over generated values.

mod decoder;
mod garbage;
mod link;
mod negotiate;
mod roundtrip;
mod support;

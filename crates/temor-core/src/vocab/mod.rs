//! The words every machine shares: identifiers, units and named text. Most of them are
//! `temor-schema`'s, named here too so the core's callers read one place.

pub mod ai;
pub(crate) use temor_schema::lenient;
pub use temor_schema::{home, ids, text};

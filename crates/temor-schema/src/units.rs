//! Sizes that cross from one crate to another: a terminal's grid (told to the pty, kept by the
//! terminal, sent on the wire) and a count of bytes.

use serde::{Deserialize, Serialize};

/// A count of bytes: a file's size, or how much of it has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteCount(pub u64);

/// A width in cells: a terminal's columns.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Cols(pub u16);

/// A height in cells: a terminal's rows.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Rows(pub u16);

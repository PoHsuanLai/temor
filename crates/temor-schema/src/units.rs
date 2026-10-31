//! Sizes that cross from one crate to another.

use serde::{Deserialize, Serialize};

/// A count of bytes: a file's size, or how much of it has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteCount(pub u64);

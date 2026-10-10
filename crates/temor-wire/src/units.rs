//! Units, each a newtype so a column count is never mistaken for a row count or a byte count.

use serde::{Deserialize, Serialize};

/// Terminal columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cols(pub u16);

/// Terminal rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rows(pub u16);

/// A terminal's size in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    /// Columns.
    pub cols: Cols,
    /// Rows.
    pub rows: Rows,
}

/// A length in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(pub u16);

/// The size of one terminal cell in pixels, which programs on the host ask for (pixel size
/// reports, image placement) and which a pseudo-terminal carries in its window size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellPx {
    /// A cell's width.
    pub width: Pixels,
    /// A cell's height.
    pub height: Pixels,
}

impl CellPx {
    /// What the helper assumed before the window sent a size: 8 by 16.
    pub const DEFAULT: Self = Self {
        width: Pixels(8),
        height: Pixels(16),
    };
}

impl Default for CellPx {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A count of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteCount(pub u64);

/// A position in a file, in bytes from its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteOffset(pub u64);

/// A span of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteRange {
    /// Where the span starts.
    pub offset: ByteOffset,
    /// How many bytes it covers.
    pub len: ByteCount,
}

/// A point in time: milliseconds since the Unix epoch, as the host's clock reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

/// A length of time, in whole seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seconds(pub u64);

/// A process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExitCode(pub i32);

/// A TCP port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Port(pub u16);

/// A process id on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pid(pub u32);

/// A line number, counted from 1 as the agent protocol counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineNumber(pub u32);

/// A number of lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineCount(pub u32);

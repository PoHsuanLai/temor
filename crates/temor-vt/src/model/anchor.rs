//! A cell the terminal keeps following: an anchor is placed at the cursor after each shell
//! integration mark (OSC 133) and read back later, wherever the content has moved to. Once the
//! scrollback drops the cell, the anchor has no point any more.

/// One anchor. Ids are never reused within a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(pub u64);

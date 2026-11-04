//! SSH hosts (§9b): what a host has been allowed.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// One answer about one thing a host may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Grant {
    /// Not asked yet: ask the first time.
    #[default]
    Ask,
    /// Allowed.
    Allowed,
    /// Refused.
    Refused,
}

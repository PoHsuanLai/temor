//! The one switch over all AI (§8.3): while it is off, nothing reaches a model and no AI row is
//! offered.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// Whether temor may use a model at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum AiSwitch {
    /// Models may be asked, when the person asks.
    #[default]
    On,
    /// Nothing reaches a model.
    Off,
}

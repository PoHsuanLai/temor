//! The keys whose meaning belongs to the shell (§4): Escape and Ctrl-C keep it in every state.
//!
//! A machine that sees one of these while the shell has focus returns a [`PassThrough`] as part
//! of its result type, not as an optional output, so no step can keep the key from the shell:
//! it may react to the key (a proposal chip goes away on Escape), but the key always goes on.

use ds_core::word::Word;

/// A key the shell owns in every state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ShellKey {
    /// Escape.
    Escape,
    /// Ctrl-C: the shell's interrupt.
    Interrupt,
}

/// The key, sent on to the shell after the machine has seen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassThrough(pub ShellKey);

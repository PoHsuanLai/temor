//! A window's probe: the screen's text, published after each repaint so a test can read what the
//! window shows without seeing its pixels. The app never sets one.

use std::sync::{Arc, Mutex};

/// The last published screen text. Cloning shares it.
#[derive(Debug, Clone, Default)]
pub struct Probe {
    text: Arc<Mutex<String>>,
}

impl Probe {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the screen as it is now.
    pub fn publish(&self, text: String) {
        let mut slot = self
            .text
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = text;
    }

    /// The screen as it was at the last repaint.
    pub fn screen(&self) -> String {
        self.text
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

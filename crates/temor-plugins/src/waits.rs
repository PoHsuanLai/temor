//! How long temor waits on a plugin: bayonet's three waits, with temor's defaults.

use std::time::Duration;

/// The waits of one request. A plugin is killed when it breaks one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waits {
    /// From starting the plugin to its greeting.
    pub hello: Duration,
    /// The longest a plugin may say nothing while a request is open; each message starts the wait
    /// again, so a stream that reports progress may run as long as it likes.
    pub silence: Duration,
    /// After a cancel, how long the plugin has to stop before it is killed.
    pub cancel_grace: Duration,
}

impl Default for Waits {
    /// bayonet's own defaults: 5 s to greet, 30 s of silence, 2 s to stop.
    fn default() -> Self {
        Waits {
            hello: Duration::from_secs(5),
            silence: Duration::from_secs(30),
            cancel_grace: Duration::from_secs(2),
        }
    }
}

impl Waits {
    /// For answers that should be quick (facts, a chosen row): the same greeting, 5 s of silence.
    pub fn quick() -> Waits {
        Waits {
            silence: Duration::from_secs(5),
            cancel_grace: Duration::from_secs(1),
            ..Waits::default()
        }
    }
}

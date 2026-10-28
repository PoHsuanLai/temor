//! Pinning a tab the way the window does it.

use super::tabs::TabsIn;
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::Relaunch;

impl TabsIn {
    /// Pins `tab` so it opens again as a shell, running nothing of its own.
    pub fn pin_as_shell(tab: TabId) -> TabsIn {
        TabsIn::Pin {
            tab,
            relaunch: Relaunch(String::new()),
        }
    }
}

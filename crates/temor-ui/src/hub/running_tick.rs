//! The sidebar's "running 2m": the words are read when the sidebar draws, so nothing else would
//! redraw them while a command runs quietly. A timer reads each tab's words and redraws the
//! window when one of them changed (each second in the first minute, then each minute or hour).

use std::time::Duration;

use dioxus::prelude::spawn;
use ds::base::time::clock::sleep;
use temor_core::vocab::ids::TabId;

use super::desk::Desk;
use super::model::Hub;

/// How often the words are read.
const TICK: Duration = Duration::from_secs(1);

impl Hub {
    /// Each tab's state words (`idle`, `running 2m`), in the tabs' order.
    pub(super) fn state_words(&self) -> Vec<(TabId, String)> {
        self.ws
            .tabs
            .place
            .tabs
            .iter()
            .filter_map(|tab| {
                let pane = self.ws.focus_of(tab.id)?;
                let (_, state) = self.tab_name_and_state(pane, &tab.title);
                Some((tab.id, state.words()))
            })
            .collect()
    }
}

/// Redraws the window when a tab's state words change, for as long as the window lives. The
/// same tick hangs up the live cards whose grace is over.
pub fn spawn_running_loop(desk: Desk) {
    spawn(async move {
        let mut shown = desk.hub.0.borrow().state_words();
        loop {
            sleep(TICK).await;
            let hung = desk.hub.0.borrow_mut().live_tick(std::time::Instant::now());
            let now = desk.hub.0.borrow().state_words();
            if hung || now != shown {
                shown = now;
                desk.bump();
            }
        }
    });
}

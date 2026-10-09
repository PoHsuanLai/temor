//! The periodic work: end sessions that have been detached too long, report changed ports, and
//! look again at watched files.

use temor_wire::hello::Capability;
use temor_wire::message::{Event, Message};

use super::core::Daemon;
use crate::idle::{IdleVerdict, idle_step};

impl Daemon {
    /// Does one round of periodic work. The caller's timer calls it about once a second; nothing
    /// else drives it.
    pub fn tick(&self) {
        self.sweep_idle();
        self.report_ports();
        self.poll_watches();
    }

    fn sweep_idle(&self) {
        let limit = *self
            .idle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = self.config.clock.now();
        for session in self.registry.all() {
            let Some(since) = session.detached_since() else {
                continue;
            };
            if idle_step(limit, since, now) == IdleVerdict::End {
                session.hang_up();
            }
        }
    }

    fn report_ports(&self) {
        // A scan that fails keeps the last report; the next tick tries again.
        let Ok(now) = self.scan_ports() else {
            return;
        };
        {
            let mut last = self
                .ports
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if *last == now {
                return;
            }
            last.clone_from(&now);
        }
        self.broadcast(Capability::Ports, &Message::Event(Event::Ports(now)));
    }

    fn poll_watches(&self) {
        for link in self.links_snapshot() {
            let watched = link
                .watches
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .paths();
            for (id, path) in watched {
                // A path that cannot be looked at (outside the folders, say) is left as it was.
                let Ok(now) = self.files.observe(&path) else {
                    continue;
                };
                let change = link
                    .watches
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .apply(id, now);
                if let Some((path, kind)) = change {
                    let event = Message::Event(Event::FileChanged {
                        watch: id,
                        path,
                        kind,
                    });
                    // A connection that cannot take it hangs itself up.
                    let _ = link.try_send(&event, None);
                }
            }
        }
    }
}

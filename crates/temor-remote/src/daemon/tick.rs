//! The periodic work: end sessions that have been detached too long, report changed ports, and
//! look again at watched files.

use temor_wire::hello::Capability;
use temor_wire::message::{Event, Message};

use super::core::Daemon;
use crate::idle::{IdleVerdict, idle_step};
use crate::sync::lock;

impl Daemon {
    /// Does one round of periodic work. The caller's timer calls it about once a second; nothing
    /// else drives it.
    pub fn tick(&self) {
        self.sweep_idle();
        self.report_ports();
        self.poll_watches();
    }

    fn sweep_idle(&self) {
        let limit = *lock(&self.idle);
        let now = self.config.clock.now();
        for session in self.registry.all() {
            let Some(since) = session.detached_since() else {
                continue;
            };
            if idle_step(limit, since, now) == IdleVerdict::End {
                // Closed, not only hung up: the pump then reaps it and kills what ignores the hangup
                // (macOS's bash does, idle at its prompt).
                session.close();
            }
        }
    }

    fn report_ports(&self) {
        // A scan that fails keeps the last report; the next tick tries again.
        let Ok(now) = self.scan_ports() else {
            return;
        };
        {
            let mut last = lock(&self.ports);
            if *last == now {
                return;
            }
            last.clone_from(&now);
        }
        self.broadcast(Capability::Ports, &Message::Event(Event::Ports(now)));
    }

    fn poll_watches(&self) {
        for link in self.links_snapshot() {
            let watched = lock(&link.watches).paths();
            for (id, path) in watched {
                // A path that cannot be looked at (outside the folders, say) is left as it was.
                let Ok(now) = self.files.observe(&path) else {
                    continue;
                };
                let change = lock(&link.watches).apply(id, now);
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

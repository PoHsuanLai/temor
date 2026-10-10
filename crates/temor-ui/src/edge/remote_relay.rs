//! The first connection of a host is made by an `ssh` in the pane, because it may ask for a
//! password or a confirmation the person answers there. Its events pass through here: the end of
//! a clean run is the news that the connection is up, and is not the pane's end (the pane then
//! becomes a session on the helper).

use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use temor_pty::ChildExit;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::remote_event::RemoteEvent;

/// What an event of the connecting pane means.
#[derive(Debug)]
pub enum Relayed {
    /// The pane's own event: on to the window as it is.
    Pane(PaneEvent),
    /// The `ssh` finished cleanly: the shared connection is up.
    Connected,
}

/// Sorts an event of the connecting pane.
pub fn relayed(event: PaneEvent) -> Relayed {
    match event {
        PaneEvent::Exited(Some(ChildExit::Exited(0))) => Relayed::Connected,
        other => Relayed::Pane(other),
    }
}

/// A sender for the connecting pane's events, and the thread that sorts them: the pane's own go
/// to `window`, the connection's news to `remote`. When the thread cannot start, everything goes
/// to the window as it is.
pub fn relay(
    window: UnboundedSender<(PaneId, PaneEvent)>,
    remote: UnboundedSender<RemoteEvent>,
) -> UnboundedSender<(PaneId, PaneEvent)> {
    let (events, mut inbox) = unbounded_channel::<(PaneId, PaneEvent)>();
    let onward = window.clone();
    let started = std::thread::Builder::new()
        .name("temor-remote-relay".to_owned())
        .spawn(move || {
            while let Some((pane, event)) = inbox.blocking_recv() {
                match relayed(event) {
                    Relayed::Pane(event) => {
                        let _ = onward.send((pane, event));
                    }
                    Relayed::Connected => {
                        let _ = remote.send(RemoteEvent::Connected { pane });
                    }
                }
            }
        });
    match started {
        Ok(_) => events,
        Err(_) => window,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_clean_exit_means_the_shared_connection_is_up() {
        let cases = [
            (Some(ChildExit::Exited(0)), true),
            (Some(ChildExit::Exited(255)), false),
            (Some(ChildExit::Signaled(9)), false),
            (None, false),
        ];
        for (exit, up) in cases {
            let sorted = relayed(PaneEvent::Exited(exit));
            assert_eq!(matches!(sorted, Relayed::Connected), up, "{exit:?}");
        }
        assert!(matches!(
            relayed(PaneEvent::Bell),
            Relayed::Pane(PaneEvent::Bell)
        ));
    }
}

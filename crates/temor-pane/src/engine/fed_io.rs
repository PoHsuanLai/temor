//! The loop of a pane fed from elsewhere: the far end's output arrives as commands, nothing is
//! polled (see `fed`).

use std::ops::ControlFlow;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, LineCount, Snapshot};

use super::event::PaneEvent;
use super::io::{Command, Io, carry_out, decide, deliver, send, serve, signal_dirty};
use super::poison::lock;
use super::term::Term;
use super::xfer::Transfers;

/// What a fed pane needs to rebuild its terminal from a snapshot.
#[derive(Debug, Clone)]
pub(super) struct FedSetup {
    pub cell: CellPx,
    pub scrollback: LineCount,
    pub cursor: CursorShape,
    pub palette: Palette,
}

/// How long a fed pane waits for a command before looking at its transfers again.
const FED_POLL: Duration = Duration::from_millis(100);

/// The loop of a pane fed from elsewhere: output arrives as commands, nothing is polled.
pub(super) fn pump_fed(mut io: Io) {
    loop {
        let sending = io.transfers.as_ref().is_some_and(|t| t.sending());
        let wait = match sending {
            true => Duration::from_millis(1),
            false => FED_POLL,
        };
        match io.commands.recv_timeout(wait) {
            Ok(command) => {
                if fed_command(&mut io, command).is_break() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Every sender is gone, the handle included: nobody is left to show it to.
            Err(RecvTimeoutError::Disconnected) => return abort_transfers(&mut io),
        }
        while let Ok(command) = io.commands.try_recv() {
            if fed_command(&mut io, command).is_break() {
                return;
            }
        }
        if !io.outbox.is_waiting() && io.transfers.as_ref().is_some_and(|t| t.sending()) {
            let reaction = io.transfers.as_mut().map(Transfers::pump_sends);
            carry_out(&mut io, reaction);
        }
    }
}

fn fed_command(io: &mut Io, command: Command) -> ControlFlow<()> {
    match command {
        Command::Shutdown => {
            abort_transfers(io);
            return ControlFlow::Break(());
        }
        Command::Decide(offer, consent) => decide(io, offer, consent),
        Command::Serve(offer, consent, shelf) => serve(io, offer, consent, &shelf),
        Command::Feed(bytes) => deliver(io, &bytes),
        Command::Restore(snapshot) => restore(io, &snapshot),
        Command::End(exit) => {
            abort_transfers(io);
            send(io, PaneEvent::Exited(exit));
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}

fn abort_transfers(io: &mut Io) {
    let aborted = io.transfers.as_mut().map(Transfers::abort_all);
    carry_out(io, aborted);
}

/// Replaces a fed pane's terminal with one that begins as `snapshot`, and tells the window the
/// screen was reset (what it knew of the old one is gone).
fn restore(io: &mut Io, snapshot: &Snapshot) {
    let Some(setup) = io.fed.clone() else {
        return;
    };
    let restored = Term::restored(
        snapshot,
        setup.cell,
        setup.scrollback,
        setup.cursor,
        &setup.palette,
    );
    // A snapshot that cannot be read leaves the old screen; the live output still follows.
    if let Ok(mut term) = restored {
        {
            let mut current = lock(&io.term);
            // The window may still hold anchors of the old terminal (where Enter was pressed
            // while the link came up): the new one numbers past them, so none names its own.
            term.number_anchors_after(&current);
            *current = term;
        }
        send(io, PaneEvent::Reset);
        signal_dirty(io);
    }
}

//! A pane whose program runs somewhere else: bytes in, bytes out, resize (PLAN §9b, Tier B).
//!
//! The pane keeps its terminal, its events and its transfers exactly as a pane on a pseudo-terminal
//! does. What differs is the program's end: the caller gives a [`PaneLink`] that takes the
//! person's input and the size, and keeps a [`Feeder`] to put the program's output (and, after a
//! reconnect, a fresh screen) into the pane. Nothing here knows what the far end is.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use temor_pty::ChildExit;
use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, GridSize, LineCount, Snapshot};

use super::event::PaneEvent;
use super::fed_io::FedSetup;
use super::handle::PaneHandle;
use super::io::{Command, Io, PaneRunner};
use super::outbox::Outbox;
use super::sink::EventSink;
use super::term::Term;
use super::watch::ForegroundWatch;
use super::xfer::{TransferSetup, Transfers};
use crate::error::PaneError;

/// Where a fed pane's input and size go. Called from the window's thread, so both must return at
/// once: queue the work, never wait on the network.
pub trait PaneLink: fmt::Debug + Send + Sync {
    /// Sends the person's input (keys, paste, a mouse report, a reply) toward the program. An
    /// error means the link is gone for good; the bytes that waited are dropped.
    fn send(&self, bytes: &[u8]) -> io::Result<()>;

    /// The grid changed: the program should learn the new size.
    fn resize(&self, grid: GridSize, cell: CellPx);
}

/// What a fed pane is made of.
#[derive(Debug)]
pub struct FedSpec {
    /// Where input and the size go.
    pub link: Arc<dyn PaneLink>,
    /// The grid in cells, and one cell in device pixels.
    pub grid: GridSize,
    pub cell: CellPx,
    pub scrollback: LineCount,
    pub palette: Palette,
    /// The cursor shape a plain reset (`CSI 0 q`) restores.
    pub cursor: CursorShape,
    /// Where files the program sends are staged, and the caps on them. `None`: the program's
    /// transfer sequences are ignored.
    pub transfers: Option<TransferSetup>,
}

/// The way into a fed pane from the far end. Cloning shares the pane. A feeder whose pane is gone
/// drops what it is given.
#[derive(Debug, Clone)]
pub struct Feeder {
    commands: Sender<Command>,
}

impl Feeder {
    /// The program wrote `bytes`.
    pub fn bytes(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.commands.send(Command::Feed(bytes));
        }
    }

    /// The pane's screen starts over as `snapshot` (the link came back and the far end sent its
    /// screen). Output given after this lands on top of it.
    pub fn restore(&self, snapshot: Snapshot) {
        let _ = self.commands.send(Command::Restore(snapshot));
    }

    /// The program ended; the pane reports it like a pane on a terminal does.
    pub fn end(&self, exit: Option<ChildExit>) {
        let _ = self.commands.send(Command::End(exit));
    }
}

/// Starts a pane fed from elsewhere. Events go to `events`. The loop is returned, not run: the
/// caller runs it on a thread of its own (see [`PaneRunner::run`]).
pub fn start_fed(
    spec: FedSpec,
    events: Arc<dyn EventSink>,
) -> Result<(PaneHandle, PaneRunner, Feeder), PaneError> {
    let term = Term::new(
        spec.grid,
        spec.cell,
        spec.scrollback,
        spec.cursor,
        &spec.palette,
    )?;
    let term = Arc::new(Mutex::new(term));
    let outbox = Arc::new(Outbox::linked(spec.link));
    let (commands_tx, commands) = channel();
    let dirty = Arc::new(AtomicBool::new(false));
    let io = Io {
        term: Arc::clone(&term),
        outbox: Arc::clone(&outbox),
        pty: None,
        fed: Some(FedSetup {
            cell: spec.cell,
            scrollback: spec.scrollback,
            cursor: spec.cursor,
            palette: spec.palette,
        }),
        commands,
        events: Arc::clone(&events),
        dirty: Arc::clone(&dirty),
        last_title: None,
        transfers: spec.transfers.map(Transfers::new),
        tap: None,
        watch: ForegroundWatch::new(),
    };
    // The first frame is always due.
    let _ = dirty.swap(true, Ordering::AcqRel);
    events.send(PaneEvent::Dirty);
    let feeder = Feeder {
        commands: commands_tx.clone(),
    };
    Ok((
        PaneHandle::new(term, outbox, commands_tx, events, dirty),
        PaneRunner::new(io),
        feeder,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::poison::lock;
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    /// A link that records what the pane sends toward the program.
    #[derive(Debug, Default)]
    struct Recording {
        sent: Mutex<Vec<u8>>,
        sizes: Mutex<Vec<(u16, u16)>>,
    }

    impl PaneLink for Recording {
        fn send(&self, bytes: &[u8]) -> io::Result<()> {
            lock(&self.sent).extend_from_slice(bytes);
            Ok(())
        }

        fn resize(&self, grid: GridSize, _cell: CellPx) {
            lock(&self.sizes).push((grid.cols.0, grid.rows.0));
        }
    }

    fn spec(link: Arc<dyn PaneLink>) -> FedSpec {
        FedSpec {
            link,
            grid: GridSize::new(20, 4),
            cell: CellPx::new(8, 16),
            scrollback: LineCount(100),
            palette: Palette::default(),
            cursor: CursorShape::Block,
            transfers: None,
        }
    }

    fn until_exited(events: &Receiver<PaneEvent>) -> Option<ChildExit> {
        loop {
            match events.recv_timeout(Duration::from_secs(5)) {
                Ok(PaneEvent::Exited(exit)) => return exit,
                Ok(_) => {}
                Err(_) => panic!("the pane never reported the end"),
            }
        }
    }

    #[test]
    fn a_fed_pane_shows_what_it_is_fed_sends_input_out_and_ends_when_told() {
        let link = Arc::new(Recording::default());
        let (events_tx, events) = channel();
        let (handle, runner, feeder) = start_fed(spec(link.clone()), Arc::new(events_tx))
            .expect("a fed pane starts without a program");
        let thread = std::thread::spawn(move || runner.run());

        feeder.bytes(b"hello".to_vec());
        feeder.end(Some(ChildExit::Exited(3)));
        assert_eq!(until_exited(&events), Some(ChildExit::Exited(3)));
        thread.join().expect("the loop ends after the program does");

        let first_row = handle
            .with_term(|term| term.row_text(0).map(|row| row.text().to_owned()))
            .expect("the first row reads");
        assert!(
            first_row.starts_with("hello"),
            "the fed bytes reached the screen: {first_row:?}"
        );

        handle.send(b"ls\r");
        handle
            .resize(GridSize::new(30, 10), CellPx::new(8, 16))
            .expect("the terminal resizes");
        assert_eq!(*link.sent.lock().expect("not poisoned"), b"ls\r");
        assert_eq!(*link.sizes.lock().expect("not poisoned"), vec![(30, 10)]);
    }

    /// An anchor taken before a restore (the cursor where Enter was pressed while the link was
    /// still coming up) must not name an anchor of the restored terminal: releasing the old one
    /// would take the new prompt's anchor with it, and its command would get no card.
    #[test]
    fn anchors_taken_after_a_restore_never_reuse_an_earlier_anchors_id() {
        let (events_tx, events) = channel();
        let (handle, runner, feeder) =
            start_fed(spec(Arc::new(Recording::default())), Arc::new(events_tx))
                .expect("a fed pane starts without a program");
        let thread = std::thread::spawn(move || runner.run());

        let before = handle
            .with_term(Term::anchor_cursor)
            .expect("the cursor anchors before the restore");
        let snapshot = handle
            .with_term(|term| term.snapshot())
            .expect("the screen encodes");
        feeder.restore(snapshot);
        loop {
            match events.recv_timeout(Duration::from_secs(5)) {
                Ok(PaneEvent::Reset) => break,
                Ok(_) => {}
                Err(_) => panic!("the pane never reported the restore"),
            }
        }

        let after = handle
            .with_term(Term::anchor_cursor)
            .expect("the cursor anchors after the restore");
        assert_ne!(after, before, "a restored terminal reused an anchor id");
        handle.with_term(|term| term.release_anchor(before));
        assert!(
            handle
                .with_term(|term| term.anchor_point(after, temor_vt::PointSpace::Screen))
                .is_some(),
            "releasing the anchor from before the restore released a new one"
        );

        feeder.end(None);
        let _ = until_exited(&events);
        thread.join().expect("the loop ends");
    }
}

//! The pane's way to its helper session: input and size go to a thread of the pane's own that
//! talks to the connection, so a key press never waits on the network (PLAN §9b).

use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};

use temor_pane::PaneLink;
use temor_vt::{CellPx, GridSize};
use temor_wire::ids::SessionId;
use temor_wire::units::{CellPx as WireCell, Size};

use super::remote_calls::{grid_size, input, resize, wire_cell};
use super::remote_conn::Conn;

/// Where a pane's input goes right now: the connection and session it is attached to, if any.
/// The size and the cell's pixels are remembered for the next attach.
#[derive(Debug)]
pub struct Target {
    pub conn: Option<Arc<Conn>>,
    pub session: Option<SessionId>,
    pub size: Size,
    pub cell: WireCell,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            conn: None,
            session: None,
            size: grid_size(80, 24),
            cell: WireCell::DEFAULT,
        }
    }
}

/// A pane's target, shared between the window's thread and the pane's writer.
pub type SharedTarget = Arc<Mutex<Target>>;

/// Locks a target; a poisoned one still holds the connection.
pub fn lock(target: &Mutex<Target>) -> MutexGuard<'_, Target> {
    target
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug)]
enum Wish {
    Input(Vec<u8>),
    Size(Size, WireCell),
}

/// The [`PaneLink`] of a helper session.
#[derive(Debug)]
pub struct RemoteLink {
    wishes: Sender<Wish>,
}

impl RemoteLink {
    /// A link to `target`, with the thread that carries what the pane says.
    pub fn start(target: SharedTarget) -> io::Result<Arc<Self>> {
        let (wishes, inbox) = channel();
        std::thread::Builder::new()
            .name("temor-remote-writer".to_owned())
            .spawn(move || carry(&inbox, &target))?;
        Ok(Arc::new(Self { wishes }))
    }
}

impl PaneLink for RemoteLink {
    fn send(&self, bytes: &[u8]) -> io::Result<()> {
        self.wishes
            .send(Wish::Input(bytes.to_vec()))
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }

    fn resize(&self, grid: GridSize, cell: CellPx) {
        // A gone writer means the pane is closing.
        let _ = self.wishes.send(Wish::Size(
            grid_size(grid.cols.0, grid.rows.0),
            wire_cell(cell),
        ));
    }
}

/// What one round of wishes comes to: the newest size with its cell, and the input in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Round {
    pub size: Option<(Size, WireCell)>,
    pub input: Vec<u8>,
}

impl Round {
    fn add(&mut self, wish: Wish) {
        match wish {
            Wish::Input(bytes) => self.input.extend_from_slice(&bytes),
            Wish::Size(size, cell) => self.size = Some((size, cell)),
        }
    }
}

/// Carries wishes to the connection until the pane's link is dropped. Input that arrives while
/// the pane is detached is dropped: the pane says so, and nothing is typed into the past.
fn carry(inbox: &Receiver<Wish>, target: &Mutex<Target>) {
    while let Ok(first) = inbox.recv() {
        let mut round = Round::default();
        round.add(first);
        while let Ok(more) = inbox.try_recv() {
            round.add(more);
        }
        let (conn, session) = {
            let mut target = lock(target);
            if let Some((size, cell)) = round.size {
                target.size = size;
                target.cell = cell;
            }
            (target.conn.clone(), target.session)
        };
        let (Some(conn), Some(session)) = (conn, session) else {
            continue;
        };
        if let Some(size) = round.size {
            // A failure here is the link dropping; the reconnect machine hears of it elsewhere.
            let _ = resize(conn.client(), session, size);
        }
        if !round.input.is_empty() {
            let _ = input(conn.client(), session, &round.input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_keeps_input_in_order_and_only_the_newest_size() {
        let cell = wire_cell(CellPx::new(9, 18));
        let mut round = Round::default();
        for wish in [
            Wish::Input(b"ls".to_vec()),
            Wish::Size(grid_size(100, 30), WireCell::DEFAULT),
            Wish::Input(b" -l\r".to_vec()),
            Wish::Size(grid_size(120, 40), cell),
        ] {
            round.add(wish);
        }
        assert_eq!(
            round,
            Round {
                size: Some((grid_size(120, 40), cell)),
                input: b"ls -l\r".to_vec()
            }
        );
    }
}

//! Starting a pane: the program on a new terminal, the emulator for it, and the I/O thread that
//! joins them. The caller builds the program's spawn spec and launcher at the app's edge.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use temor_pty::{Cols, Pty, Px as PtyPx, Rows, SessionLauncher, SpawnSpec};
use temor_render::Palette;
use temor_vt::{CellPx, GridSize, LineCount};
use tokio::sync::mpsc::UnboundedSender;

use super::event::PaneEvent;
use super::handle::PaneHandle;
use super::io::{Io, run};
use super::outbox::Outbox;
use super::term::Term;
use crate::error::UiError;

/// What a new pane is made of.
#[derive(Debug)]
pub struct PaneSpec {
    /// The program, its arguments, directory and complete environment.
    pub spawn: SpawnSpec,
    /// How the program becomes a session leader (PLAN §6, temor-pty).
    pub launcher: SessionLauncher,
    /// The grid in cells, and one cell in device pixels.
    pub grid: GridSize,
    pub cell: CellPx,
    pub scrollback: LineCount,
    pub palette: Palette,
    /// The cursor shape a plain reset (`CSI 0 q`) restores.
    pub cursor: temor_vt::CursorShape,
}

/// The pseudo-terminal's size for a grid of cells, in pixels too (for programs that draw).
pub(super) fn pty_size(grid: GridSize, cell: CellPx) -> temor_pty::GridSize {
    temor_pty::GridSize {
        cols: Cols(grid.cols.0),
        rows: Rows(grid.rows.0),
        width: PtyPx(saturate(u32::from(grid.cols.0) * cell.width.0)),
        height: PtyPx(saturate(u32::from(grid.rows.0) * cell.height.0)),
    }
}

fn saturate(value: u32) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// Starts the program and its I/O thread. Events for the window go to `events`.
pub fn start(spec: PaneSpec, events: UnboundedSender<PaneEvent>) -> Result<PaneHandle, UiError> {
    let size = pty_size(spec.grid, spec.cell);
    let spawn = SpawnSpec { size, ..spec.spawn };
    let pty = Pty::spawn(&spawn, &spec.launcher)?;
    let port = pty.io()?;
    let term = Term::new(
        spec.grid,
        spec.cell,
        spec.scrollback,
        spec.cursor,
        &spec.palette,
    )?;
    let term = Arc::new(Mutex::new(term));
    let outbox = Arc::new(Outbox::new(port));
    let (commands_tx, commands) = channel();
    let dirty = Arc::new(AtomicBool::new(false));
    let io = Io {
        term: Arc::clone(&term),
        outbox: Arc::clone(&outbox),
        pty,
        commands,
        events: events.clone(),
        dirty: Arc::clone(&dirty),
        last_title: None,
    };
    std::thread::Builder::new()
        .name("temor-pane-io".to_owned())
        .spawn(move || run(io))
        .map_err(UiError::Thread)?;
    // The first frame is always due.
    let _ = dirty.swap(true, std::sync::atomic::Ordering::AcqRel);
    let _ = events.send(PaneEvent::Dirty);
    Ok(PaneHandle::new(term, outbox, commands_tx, events, dirty))
}

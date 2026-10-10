//! Starting a pane: the program on a new terminal, the emulator for it, and the I/O thread that
//! joins them. The caller builds the program's spawn spec and launcher at the app's edge.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use temor_pty::{Cols, Pty, Px as PtyPx, Rows, SessionLauncher, SpawnSpec};
use temor_render::Palette;
use temor_vt::{CellPx, GridSize, LineCount, Snapshot};

use super::event::PaneEvent;
use super::handle::PaneHandle;
use super::io::{Io, PaneRunner};
use super::outbox::Outbox;
use super::sink::EventSink;
use super::term::Term;
use super::xfer::{TransferSetup, Transfers};
use super::watch::ForegroundWatch;
use crate::error::PaneError;

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
    /// Where files the program sends are staged, and the caps on them. `None`: the program's
    /// transfer sequences are ignored.
    pub transfers: Option<TransferSetup>,
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

/// Starts the program and its I/O loop. Events for the window go to `events`. The loop is
/// returned, not run: the caller runs it on a thread of its own (see [`PaneRunner::run`]).
pub fn start(
    spec: PaneSpec,
    events: Arc<dyn EventSink>,
) -> Result<(PaneHandle, PaneRunner), PaneError> {
    start_from(spec, None, events)
}

/// Like [`start`], but the terminal begins as `snapshot` (a closed pane's screen and
/// scrollback), with the new program writing below it.
pub fn start_restored(
    spec: PaneSpec,
    snapshot: &Snapshot,
    events: Arc<dyn EventSink>,
) -> Result<(PaneHandle, PaneRunner), PaneError> {
    start_from(spec, Some(snapshot), events)
}

fn start_from(
    spec: PaneSpec,
    snapshot: Option<&Snapshot>,
    events: Arc<dyn EventSink>,
) -> Result<(PaneHandle, PaneRunner), PaneError> {
    let size = pty_size(spec.grid, spec.cell);
    let spawn = SpawnSpec { size, ..spec.spawn };
    let pty = Pty::spawn(&spawn, &spec.launcher)?;
    let port = pty.io()?;
    let term = match snapshot {
        Some(snapshot) => Term::restored(
            snapshot,
            spec.cell,
            spec.scrollback,
            spec.cursor,
            &spec.palette,
        )?,
        None => Term::new(
            spec.grid,
            spec.cell,
            spec.scrollback,
            spec.cursor,
            &spec.palette,
        )?,
    };
    let term = Arc::new(Mutex::new(term));
    let outbox = Arc::new(Outbox::new(port));
    let (commands_tx, commands) = channel();
    let dirty = Arc::new(AtomicBool::new(false));
    let io = Io {
        term: Arc::clone(&term),
        outbox: Arc::clone(&outbox),
        pty,
        commands,
        events: Arc::clone(&events),
        dirty: Arc::clone(&dirty),
        last_title: None,
        transfers: spec.transfers.map(Transfers::new),
        watch: ForegroundWatch::new(),
    };
    // The first frame is always due.
    let _ = dirty.swap(true, Ordering::AcqRel);
    events.send(PaneEvent::Dirty);
    Ok((
        PaneHandle::new(term, outbox, commands_tx, events, dirty),
        PaneRunner::new(io),
    ))
}

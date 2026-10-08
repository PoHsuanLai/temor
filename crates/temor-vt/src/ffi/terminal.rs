//! `Terminal`: one libghostty-vt terminal, its callbacks and the inline-file pre-scan.

use std::cell::Cell;
use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::callbacks::{self, Callbacks, TitleChange};
use super::convert::{check, mode_bits};
use super::png::install_png_decoder;
use super::snapshot::Snapshot;
use super::terminal_read::raw_size;
use crate::scan::{InlineFileScanner, Scanned};
use crate::{
    CellPx, EventSink, GridSize, ModeState, ReplySink, TerminalConfig, TerminalMode, UnknownKind,
    ViewportScroll, VtError, VtEvent,
};

/// A terminal emulator backed by libghostty-vt. Feed it program output with
/// [`Terminal::write`]; what the output asks of the host arrives as [`VtEvent`]s on the event
/// sink, and the terminal's answers to the program go to the [`ReplySink`], both given at
/// creation. Mode 2027 (grapheme clusters) is on by default.
///
/// Share it between the I/O thread and the renderer as `Mutex<Terminal>`: it is `Send`, and
/// [`crate::RenderView`] only needs the lock for `begin`.
pub struct Terminal {
    raw: NonNull<sys::TerminalImpl>,
    /// Boxed so `userdata` stays valid; freed in `Drop` after the terminal.
    callbacks: NonNull<Callbacks>,
    scanner: InlineFileScanner,
}

// SAFETY: a `Terminal` may move to another thread because
// - libghostty-vt keeps no thread-local state in the terminal path (P0(a): no `threadlocal` in
//   src/terminal, src/unicode or src/simd), and only requires that calls do not overlap, which
//   `&`/`&mut` (and the caller's `Mutex`) guarantee since `Terminal` is not `Sync`;
// - every callback runs on the thread that calls `write`, and only touches `Callbacks`, whose
//   fields are `Send`: the sinks are `Box<dyn EventSink>`/`Box<dyn ReplySink>`, both traits
//   requiring `Send`, and the `Cell`s
//   hold `Copy` values. No callback can be registered except through this module, so no
//   non-`Send` closure ever reaches libghostty;
// - the PNG decoder hook is a stateless function.
unsafe impl Send for Terminal {}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl Terminal {
    /// A blank terminal of `config.size` sending its events to `sink` and its replies to
    /// `replies`.
    pub fn new(
        config: TerminalConfig,
        sink: Box<dyn EventSink>,
        replies: Box<dyn ReplySink>,
    ) -> Result<Self, VtError> {
        let mut raw = std::ptr::null_mut();
        let (cols, rows) = (config.size.cols.0, config.size.rows.0);
        // SAFETY: `raw` is a valid out-pointer; a null allocator selects the default one.
        check(
            unsafe { sys::ghostty_terminal_new(std::ptr::null(), &mut raw, cols, rows) },
            "size",
        )?;
        let term = Self::adopt(raw, config, sink, replies)?;
        term.set_mode_default(TerminalMode::GraphemeCluster, ModeState::Set)?;
        Ok(term)
    }

    /// A terminal restored from `snapshot`, with its screen, scrollback, modes, cursor and title,
    /// sending its events to `sink` and replies to `replies`. `config.size` is ignored: the
    /// snapshot's size wins.
    pub fn restore(
        snapshot: &Snapshot,
        config: TerminalConfig,
        sink: Box<dyn EventSink>,
        replies: Box<dyn ReplySink>,
    ) -> Result<Self, VtError> {
        let raw = snapshot.decode()?;
        let mut config = config;
        // SAFETY: `raw` is a live terminal the decoder just handed over.
        config.size = unsafe { raw_size(raw) };
        let term = Self::adopt(raw, config, sink, replies)?;
        // Keep grapheme clusters as the reset default without changing what the program chose.
        let current = term.mode(TerminalMode::GraphemeCluster)?;
        term.set_mode_default(TerminalMode::GraphemeCluster, ModeState::Set)?;
        term.set_mode_raw(TerminalMode::GraphemeCluster, current)?;
        Ok(term)
    }

    /// Takes ownership of `raw`, installs the callbacks and applies `config`.
    fn adopt(
        raw: sys::Terminal,
        config: TerminalConfig,
        sink: Box<dyn EventSink>,
        replies: Box<dyn ReplySink>,
    ) -> Result<Self, VtError> {
        let raw = NonNull::new(raw).ok_or(VtError::OutOfMemory)?;
        let callbacks = Box::new(Callbacks {
            sink,
            replies,
            dropped: Cell::new(0),
            color_scheme: Cell::new(config.color_scheme),
            size: Cell::new((config.size, config.cell_px)),
            title: Cell::new(TitleChange::Unchanged),
        });
        let callbacks = NonNull::from(Box::leak(callbacks));
        let mut term = Self {
            raw,
            callbacks,
            scanner: InlineFileScanner::new(config.inline_file_max),
        };
        install_png_decoder();
        // SAFETY: the terminal is live and `callbacks` is boxed, never moves, and is freed only
        // after the terminal in `Drop`.
        unsafe { callbacks::install(term.raw(), term.callbacks.as_ptr()) };
        term.set_usize(sys::TerminalOption::UNKNOWN_MAX_BYTES, config.unknown_max.0)?;
        term.set_usize(
            sys::TerminalOption::SCROLLBACK_MAX_LINES,
            config.scrollback.0,
        )?;
        let kitty = config.kitty_image_storage.0 as u64;
        // SAFETY: the option takes a `uint64_t*`, valid for the call.
        check(
            unsafe {
                sys::ghostty_terminal_set(
                    term.raw(),
                    sys::TerminalOption::KITTY_IMAGE_STORAGE_LIMIT,
                    (&raw const kitty).cast(),
                )
            },
            "kitty image storage limit",
        )?;
        term.resize(config.size, config.cell_px)?;
        Ok(term)
    }

    pub(super) fn raw(&self) -> sys::Terminal {
        self.raw.as_ptr()
    }

    pub(super) fn callbacks(&self) -> &Callbacks {
        // SAFETY: `callbacks` is boxed, owned by `self` and only accessed through shared refs.
        unsafe { self.callbacks.as_ref() }
    }

    fn set_usize(&self, option: sys::TerminalOption::Type, value: usize) -> Result<(), VtError> {
        // SAFETY: the option takes a `size_t*`, valid for the call.
        check(
            unsafe { sys::ghostty_terminal_set(self.raw(), option, (&raw const value).cast()) },
            "limit",
        )
    }

    /// Feeds program output. Inline files (OSC 1337 `File=`) are taken out first and reported as
    /// [`UnknownKind::Osc1337File`]; everything else goes to libghostty, whose callbacks send
    /// their events before this returns.
    pub fn write(&mut self, bytes: &[u8]) {
        let raw = self.raw();
        let cb = self.callbacks;
        self.scanner.feed(bytes, |seg| match seg {
            // SAFETY: the terminal is live and `b` is readable for the call. Callbacks may run;
            // they only use `Callbacks` through shared refs.
            Scanned::Bytes(b) => unsafe {
                sys::ghostty_terminal_vt_write(raw, b.as_ptr(), b.len())
            },
            Scanned::File {
                payload,
                truncation,
            } => {
                // SAFETY: as in `callbacks()`.
                let cb = unsafe { cb.as_ref() };
                cb.sink.send(VtEvent::Unknown {
                    kind: UnknownKind::Osc1337File,
                    payload,
                    truncation,
                });
            }
        });
        if self.callbacks().title.replace(TitleChange::Unchanged) == TitleChange::Changed {
            let title = self.title();
            self.callbacks().sink.send(VtEvent::Title(title));
        }
    }

    /// Changes the grid and cell size, reflowing soft-wrapped lines. Like `write`, it can send
    /// events (a resize ends a render hold) and replies (in-band size reports, mode 2048).
    pub fn resize(&mut self, size: GridSize, cell: CellPx) -> Result<(), VtError> {
        // Size reports sent from inside the resize must already see the new size.
        let before = self.callbacks().size.replace((size, cell));
        // SAFETY: the terminal is live.
        check(
            unsafe {
                sys::ghostty_terminal_resize(
                    self.raw(),
                    size.cols.0,
                    size.rows.0,
                    cell.width.0,
                    cell.height.0,
                )
            },
            "size",
        )
        .inspect_err(|_| self.callbacks().size.set(before))
    }

    /// Moves the viewport over the scrollback.
    pub fn scroll_viewport(&mut self, to: ViewportScroll) {
        use sys::TerminalScrollViewportTag as T;
        let mut value = sys::TerminalScrollViewportValue::default();
        let tag = match to {
            ViewportScroll::Top => T::TOP,
            ViewportScroll::Bottom => T::BOTTOM,
            ViewportScroll::Delta(d) => {
                value.delta = d.0;
                T::DELTA
            }
            ViewportScroll::Row(r) => {
                value.row = usize::try_from(r.0).unwrap_or(usize::MAX);
                T::ROW
            }
        };
        // SAFETY: the terminal is live and the tag names the union member that was set.
        unsafe {
            sys::ghostty_terminal_scroll_viewport(
                self.raw(),
                sys::TerminalScrollViewport { tag, value },
            )
        };
    }

    /// Whether `mode` is on.
    pub fn mode(&self, mode: TerminalMode) -> Result<ModeState, VtError> {
        let mut config = sys::TerminalModeConfig {
            mode: mode_bits(mode),
            value: false,
        };
        // SAFETY: DATA_MODE reads and writes a `GhosttyTerminalModeConfig`.
        check(
            unsafe {
                sys::ghostty_terminal_get(
                    self.raw(),
                    sys::TerminalData::MODE,
                    (&raw mut config).cast(),
                )
            },
            "mode",
        )?;
        Ok(if config.value {
            ModeState::Set
        } else {
            ModeState::Reset
        })
    }

    /// Turns `mode` on or off, as if the program had (no reply is sent).
    pub fn set_mode(&mut self, mode: TerminalMode, state: ModeState) -> Result<(), VtError> {
        self.set_mode_raw(mode, state)
    }

    fn set_mode_raw(&self, mode: TerminalMode, state: ModeState) -> Result<(), VtError> {
        self.set_mode_option(sys::TerminalOption::MODE, mode, state)
    }

    fn set_mode_default(&self, mode: TerminalMode, state: ModeState) -> Result<(), VtError> {
        self.set_mode_option(sys::TerminalOption::MODE_DEFAULT, mode, state)
    }

    fn set_mode_option(
        &self,
        option: sys::TerminalOption::Type,
        mode: TerminalMode,
        state: ModeState,
    ) -> Result<(), VtError> {
        let config = sys::TerminalModeConfig {
            mode: mode_bits(mode),
            value: state.is_set(),
        };
        // SAFETY: MODE and MODE_DEFAULT take a `GhosttyTerminalModeConfig*`, valid for the call.
        check(
            unsafe { sys::ghostty_terminal_set(self.raw(), option, (&raw const config).cast()) },
            "mode",
        )
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: the terminal is freed first, so no callback can run afterwards; then the
        // callbacks box, leaked in `adopt`, is reclaimed exactly once.
        unsafe {
            sys::ghostty_terminal_free(self.raw());
            drop(Box::from_raw(self.callbacks.as_ptr()));
        }
    }
}

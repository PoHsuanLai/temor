//! `Terminal`: one libghostty-vt terminal, its callbacks and the inline-file pre-scan.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::anchor::Anchors;
use super::callbacks::{self, Callbacks, TitleChange};
use super::convert::{check, mode_bits};
use super::png::install_png_decoder;
use super::snapshot::Snapshot;
use super::terminal_read::raw_size;
use crate::scan::{HoldCuts, InlineFileScanner, MarkCuts};
use crate::{
    CellPx, EventSink, GridSize, ModeState, RenderHold, ReplySink, TerminalConfig, TerminalMode,
    ViewportScroll, VtError,
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
    pub(super) scanner: InlineFileScanner,
    /// Where OSC 133 marks end, carried across writes.
    pub(super) marks: MarkCuts,
    /// Where synchronized updates start, carried across writes.
    pub(super) holds: HoldCuts,
    /// Captures the frame when a hold begins.
    pub(super) hold_hook: Option<Box<dyn super::HoldHook>>,
    /// The number of the last mark cut (see `VtEvent::MarkBegin`).
    pub(super) mark_seq: u64,
    pub(super) anchors: Anchors,
}

// SAFETY: a `Terminal` may move to another thread because
// - libghostty-vt keeps no thread-local state in the terminal path (P0(a): no `threadlocal` in
//   src/terminal, src/unicode or src/simd), and only requires that calls do not overlap, which
//   `&`/`&mut` (and the caller's `Mutex`) guarantee since `Terminal` is not `Sync`;
// - every callback runs on the thread that calls `write`, and only touches `Callbacks`, whose
//   fields are `Send`: the sinks are `Box<dyn EventSink>`/`Box<dyn ReplySink>`, both traits
//   requiring `Send`, the clipboard source (also `Send`, in a `RefCell` only touched inside
//   callbacks and `set_clipboard_source`), and the `Cell`s hold `Copy` values. No callback can be registered except through this
//   module, so no non-`Send` closure ever reaches libghostty;
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

    /// Sets who answers the program's clipboard reads (OSC 52 with `?`). Without one every read
    /// is refused. The source is called from inside `write`, on the writer's thread.
    pub fn set_clipboard_source(&mut self, source: Box<dyn crate::ClipboardSource>) {
        *self.callbacks().clipboard.borrow_mut() = Some(source);
    }

    /// Sets the hook that captures a frame when a render hold begins.
    pub fn set_hold_hook(&mut self, hook: Box<dyn super::HoldHook>) {
        self.hold_hook = Some(hook);
    }

    /// Whether the program holds the screen (synchronized output) right now.
    pub fn render_hold(&self) -> RenderHold {
        self.callbacks().hold.get()
    }

    /// Ends the program's hold from the host's side (its timeout ran out): synchronized output is
    /// turned off, as libghostty advises, and updates show again. The program may start another.
    pub fn end_render_hold(&mut self) -> Result<(), VtError> {
        self.set_mode_raw(TerminalMode::SyncOutput, ModeState::Reset)?;
        // Changing the mode ourselves does not call the hold callback.
        self.callbacks().hold.set(RenderHold::Released);
        Ok(())
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
            clipboard: RefCell::new(None),
            hold: Cell::new(RenderHold::Released),
        });
        let callbacks = NonNull::from(Box::leak(callbacks));
        let mut term = Self {
            raw,
            callbacks,
            scanner: InlineFileScanner::new(config.inline_file_max),
            marks: MarkCuts::new(),
            holds: HoldCuts::new(),
            hold_hook: None,
            mark_seq: 0,
            anchors: Anchors::default(),
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

    pub(super) fn anchors(&self) -> &Anchors {
        &self.anchors
    }

    pub(super) fn anchors_mut(&mut self) -> &mut Anchors {
        &mut self.anchors
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

    /// The cell size in pixels the terminal was last given.
    pub fn cell_px(&self) -> CellPx {
        self.callbacks().size.get().1
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
            // The tracked references go first: they belong to this terminal.
            self.anchors.free_all();
            sys::ghostty_terminal_free(self.raw());
            drop(Box::from_raw(self.callbacks.as_ptr()));
        }
    }
}

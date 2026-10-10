//! The state libghostty's callbacks share, the panic guard around them, and their installation.
//!
//! Every callback runs inside `ghostty_terminal_vt_write` (or `resize`), on the thread that
//! called it, with `userdata` pointing at the terminal's [`Callbacks`]. They only read their
//! arguments, call the sinks and touch `Cell`s, so they never re-enter the terminal for writing.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use temor_vt_sys as sys;

use super::clipboard_read::on_clipboard_read;
use super::reports::{on_notification, on_program_status, on_progress, on_prompt};
use super::signals::{
    on_bell, on_clipboard, on_color_scheme, on_pwd, on_render_hold, on_reset, on_size, on_title,
    on_unknown, on_write_pty,
};
use crate::{CellPx, ClipboardSource, ColorScheme, EventSink, GridSize, ReplySink, VtEvent};

/// Whether the title changed during the current write; libghostty only lets the title be read
/// after its callback returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TitleChange {
    Unchanged,
    Changed,
}

/// The state callbacks see through `userdata`. Owned by the terminal (boxed, so its address is
/// stable) and only touched by the thread that holds the terminal.
pub(super) struct Callbacks {
    pub(super) sink: Box<dyn EventSink>,
    pub(super) replies: Box<dyn ReplySink>,
    pub(super) color_scheme: Cell<ColorScheme>,
    pub(super) size: Cell<(GridSize, CellPx)>,
    pub(super) title: Cell<TitleChange>,
    /// Answers clipboard reads; none refuses them all. Set before output is written.
    pub(super) clipboard: RefCell<Option<Box<dyn ClipboardSource>>>,
    /// Callbacks that panicked (in a sink or in their own body); their event was dropped.
    pub(super) dropped: Cell<u64>,
}

impl Callbacks {
    pub(super) fn send(&self, event: VtEvent) {
        self.sink.send(event);
    }
}

/// Runs a callback body with the terminal's `Callbacks`. A panic must not unwind into C (that
/// aborts the process), so it is caught here: the event is dropped, the drop counted, and
/// `fallback` returned to libghostty.
///
/// # Safety
/// `userdata` must be the pointer installed by [`install`], still owned by a live terminal.
pub(super) unsafe fn guarded<R>(
    userdata: *mut c_void,
    fallback: R,
    body: impl FnOnce(&Callbacks) -> R,
) -> R {
    // SAFETY: per the contract, `userdata` is a live, boxed `Callbacks` that is only ever
    // accessed through shared references.
    let cb = unsafe { &*userdata.cast::<Callbacks>() };
    // The `Cell`s hold plain `Copy` values, so a panic cannot leave them half-updated.
    match catch_unwind(AssertUnwindSafe(|| body(cb))) {
        Ok(r) => r,
        Err(_) => {
            cb.dropped.set(cb.dropped.get().saturating_add(1));
            fallback
        }
    }
}

/// Installs every callback on `term`, with `state` as userdata. Each function is first bound to
/// its bindgen typedef, so a signature change upstream is a compile error here.
///
/// # Safety
/// `term` must be a live terminal and `state` must outlive it (or until the callbacks are
/// replaced), and must not move.
pub(super) unsafe fn install(term: sys::Terminal, state: *const Callbacks) {
    use sys::TerminalOption as O;
    let write_pty: sys::TerminalWritePtyFn = Some(on_write_pty);
    let bell: sys::TerminalBellFn = Some(on_bell);
    let title: sys::TerminalTitleChangedFn = Some(on_title);
    let pwd: sys::TerminalPwdChangedFn = Some(on_pwd);
    let reset: sys::TerminalResetFn = Some(on_reset);
    let clipboard: sys::TerminalClipboardWriteFn = Some(on_clipboard);
    let clipboard_read: sys::TerminalClipboardReadFn = Some(on_clipboard_read);
    let notification: sys::TerminalDesktopNotificationFn = Some(on_notification);
    let progress: sys::TerminalProgressReportFn = Some(on_progress);
    let status: sys::TerminalProgramStatusFn = Some(on_program_status);
    let prompt: sys::TerminalSemanticPromptFn = Some(on_prompt);
    let hold: sys::TerminalRenderHoldFn = Some(on_render_hold);
    let scheme: sys::TerminalColorSchemeFn = Some(on_color_scheme);
    let size: sys::TerminalSizeFn = Some(on_size);
    let unknown: sys::TerminalUnknownSequenceFn = Some(on_unknown);
    let fns: [(sys::TerminalOption::Type, *const c_void); 15] = [
        (O::WRITE_PTY, fn_ptr(write_pty)),
        (O::BELL, fn_ptr(bell)),
        (O::TITLE_CHANGED, fn_ptr(title)),
        (O::PWD_CHANGED, fn_ptr(pwd)),
        (O::RESET, fn_ptr(reset)),
        (O::CLIPBOARD_WRITE, fn_ptr(clipboard)),
        (O::CLIPBOARD_READ, fn_ptr(clipboard_read)),
        (O::DESKTOP_NOTIFICATION, fn_ptr(notification)),
        (O::PROGRESS_REPORT, fn_ptr(progress)),
        (O::PROGRAM_STATUS, fn_ptr(status)),
        (O::SEMANTIC_PROMPT, fn_ptr(prompt)),
        (O::RENDER_HOLD, fn_ptr(hold)),
        (O::COLOR_SCHEME, fn_ptr(scheme)),
        (O::SIZE, fn_ptr(size)),
        (O::UNKNOWN_SEQUENCE, fn_ptr(unknown)),
    ];
    // SAFETY: `term` is live; USERDATA takes the pointer itself, and every callback option takes
    // a function pointer of the typedef it was bound to above.
    unsafe {
        sys::ghostty_terminal_set(term, O::USERDATA, state.cast());
        for (option, f) in fns {
            sys::ghostty_terminal_set(term, option, f);
        }
    }
}

/// The `const void*` libghostty's setter takes for a callback option.
pub(super) fn fn_ptr<F: Copy>(f: Option<F>) -> *const c_void {
    const { assert!(size_of::<F>() == size_of::<*const c_void>()) };
    match f {
        // SAFETY: `F` is a bindgen `unsafe extern "C" fn` type, pointer-sized (checked above).
        Some(f) => unsafe { std::mem::transmute_copy::<F, *const c_void>(&f) },
        None => std::ptr::null(),
    }
}

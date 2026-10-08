//! libghostty's callbacks, turned into [`VtEvent`]s on the terminal's sink.
//!
//! Every callback runs inside `ghostty_terminal_vt_write`, on the thread that called it, with
//! `userdata` pointing at the terminal's [`Callbacks`]. They only read their arguments, send on
//! the sink and touch `Cell`s, so they never block and never re-enter the terminal for writing.

use std::cell::Cell;
use std::ffi::c_void;

use temor_vt_sys as sys;

use super::convert::{bytes, string};
use super::reports::{on_notification, on_program_status, on_progress, on_prompt};
use crate::{
    CellPx, ClipboardGrant, ClipboardItem, ClipboardLocation, ClipboardWrite, ColorScheme,
    EventSink, GridSize, RenderHold, Truncation, UnknownKind, VtEvent,
};

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
    pub(super) color_scheme: Cell<ColorScheme>,
    pub(super) size: Cell<(GridSize, CellPx)>,
    pub(super) title: Cell<TitleChange>,
}

impl Callbacks {
    /// # Safety
    /// `userdata` must be the pointer installed by `install`, still owned by a live terminal.
    pub(super) unsafe fn from<'a>(userdata: *mut c_void) -> &'a Callbacks {
        // SAFETY: per the contract, `userdata` is a live, boxed `Callbacks` that is only ever
        // accessed through shared references.
        unsafe { &*userdata.cast::<Callbacks>() }
    }

    pub(super) fn send(&self, event: VtEvent) {
        self.sink.send(event);
    }
}

/// Installs every callback on `term`, with `state` as userdata.
///
/// # Safety
/// `term` must be a live terminal and `state` must outlive it (or until the callbacks are
/// replaced), and must not move.
pub(super) unsafe fn install(term: sys::Terminal, state: *const Callbacks) {
    use sys::TerminalOption as O;
    let fns: [(sys::TerminalOption::Type, *const c_void); 13] = [
        (O::WRITE_PTY, on_write_pty as *const c_void),
        (O::BELL, on_bell as *const c_void),
        (O::TITLE_CHANGED, on_title as *const c_void),
        (O::PWD_CHANGED, on_pwd as *const c_void),
        (O::CLIPBOARD_WRITE, on_clipboard as *const c_void),
        (O::DESKTOP_NOTIFICATION, on_notification as *const c_void),
        (O::PROGRESS_REPORT, on_progress as *const c_void),
        (O::PROGRAM_STATUS, on_program_status as *const c_void),
        (O::SEMANTIC_PROMPT, on_prompt as *const c_void),
        (O::RENDER_HOLD, on_render_hold as *const c_void),
        (O::COLOR_SCHEME, on_color_scheme as *const c_void),
        (O::SIZE, on_size as *const c_void),
        (O::UNKNOWN_SEQUENCE, on_unknown as *const c_void),
    ];
    // SAFETY: `term` is live; USERDATA takes the pointer itself, and every callback option takes
    // a function pointer whose signature matches the option's typedef.
    unsafe {
        sys::ghostty_terminal_set(term, O::USERDATA, state.cast());
        for (option, f) in fns {
            sys::ghostty_terminal_set(term, option, f);
        }
    }
}

unsafe extern "C" fn on_write_pty(_t: sys::Terminal, ud: *mut c_void, data: *const u8, len: usize) {
    if data.is_null() || len == 0 {
        return;
    }
    // SAFETY: libghostty passes `len` readable bytes valid for this call; `ud` is ours.
    let (cb, reply) = unsafe { (Callbacks::from(ud), std::slice::from_raw_parts(data, len)) };
    cb.send(VtEvent::Reply(reply.to_vec()));
}

unsafe extern "C" fn on_bell(_t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: `ud` is ours.
    unsafe { Callbacks::from(ud) }.send(VtEvent::Bell);
}

unsafe extern "C" fn on_title(_t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: `ud` is ours. The title is read after `vt_write` returns, as libghostty requires.
    unsafe { Callbacks::from(ud) }
        .title
        .set(TitleChange::Changed);
}

unsafe extern "C" fn on_pwd(t: sys::Terminal, ud: *mut c_void) {
    let mut pwd = sys::String {
        ptr: std::ptr::null(),
        len: 0,
    };
    // SAFETY: libghostty allows reading PWD inside this callback; the string is borrowed until
    // the next mutating call, and is copied before returning. `ud` is ours.
    let (cb, pwd) = unsafe {
        sys::ghostty_terminal_get(t, sys::TerminalData::PWD, (&raw mut pwd).cast());
        (Callbacks::from(ud), string(pwd))
    };
    cb.send(VtEvent::Cwd(pwd));
}

unsafe extern "C" fn on_clipboard(
    _t: sys::Terminal,
    ud: *mut c_void,
    w: *const sys::ClipboardWrite,
) {
    if w.is_null() {
        return;
    }
    // SAFETY: `w` and everything it points to are valid for this call; `contents` holds
    // `contents_len` entries. `ud` is ours.
    let (cb, w) = unsafe { (Callbacks::from(ud), &*w) };
    let items = if w.contents.is_null() {
        Vec::new()
    } else {
        // SAFETY: as above.
        let contents = unsafe { std::slice::from_raw_parts(w.contents, w.contents_len) };
        contents
            .iter()
            // SAFETY: both strings are borrowed for this call and copied here.
            .map(|c| unsafe {
                ClipboardItem {
                    mime: string(c.mime),
                    data: bytes(&c.data).to_vec(),
                }
            })
            .collect()
    };
    let location = match w.location {
        sys::ClipboardLocation::SELECTION => ClipboardLocation::Selection,
        sys::ClipboardLocation::PRIMARY => ClipboardLocation::Primary,
        _ => ClipboardLocation::Standard,
    };
    let grant = if w.granted {
        ClipboardGrant::Granted
    } else {
        ClipboardGrant::Ask
    };
    // SAFETY: `name` is borrowed for this call.
    let program = unsafe { string(w.name) };
    cb.send(VtEvent::Clipboard {
        write: ClipboardWrite {
            location,
            items,
            program,
            grant,
        },
    });
    // Not replying denies the write; the host decides later, so the program is told it worked.
    let mut reply = temor_vt_sys::sized!(sys::ClipboardWriteReply);
    reply.result = sys::ClipboardWriteResult::SUCCESS;
    if let Some(answer) = w.reply {
        // SAFETY: `reply` is the function libghostty gave for this request, called within it.
        unsafe { answer(w, &reply) };
    }
}

unsafe extern "C" fn on_render_hold(_t: sys::Terminal, ud: *mut c_void, held: bool) {
    let hold = if held {
        RenderHold::Held
    } else {
        RenderHold::Released
    };
    // SAFETY: `ud` is ours.
    unsafe { Callbacks::from(ud) }.send(VtEvent::RenderHold(hold));
}

unsafe extern "C" fn on_color_scheme(
    _t: sys::Terminal,
    ud: *mut c_void,
    out: *mut sys::ColorScheme::Type,
) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY: `ud` is ours; `out` is a valid out-pointer for this call.
    let cb = unsafe { Callbacks::from(ud) };
    let scheme = match cb.color_scheme.get() {
        ColorScheme::Light => sys::ColorScheme::LIGHT,
        ColorScheme::Dark => sys::ColorScheme::DARK,
    };
    // SAFETY: as above.
    unsafe { out.write(scheme) };
    cb.send(VtEvent::ColorScheme);
    true
}

unsafe extern "C" fn on_size(
    _t: sys::Terminal,
    ud: *mut c_void,
    out: *mut sys::SizeReportSize,
) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY: `ud` is ours; `out` is a valid out-pointer for this call.
    let (size, cell) = unsafe { Callbacks::from(ud) }.size.get();
    let report = sys::SizeReportSize {
        rows: size.rows.0,
        columns: size.cols.0,
        cell_width: cell.width.0,
        cell_height: cell.height.0,
    };
    // SAFETY: as above.
    unsafe { out.write(report) };
    true
}

unsafe extern "C" fn on_unknown(
    _t: sys::Terminal,
    ud: *mut c_void,
    q: *const sys::TerminalUnknownSequence,
) {
    use sys::TerminalUnknownSequenceTag as Tag;
    if q.is_null() {
        return;
    }
    // SAFETY: `q` is valid for this call and `tag` says which union member is live; the content
    // is borrowed for this call and copied here. `ud` is ours.
    let (cb, kind, payload, truncated) = unsafe {
        let q = &*q;
        let (kind, truncated, content) = match q.tag {
            Tag::GHOSTTY_TERMINAL_UNKNOWN_SEQUENCE_APC => {
                (UnknownKind::Apc, q.value.apc.truncated, q.value.apc.content)
            }
            Tag::GHOSTTY_TERMINAL_UNKNOWN_SEQUENCE_OSC => {
                (UnknownKind::Osc, q.value.osc.truncated, q.value.osc.content)
            }
            _ => return,
        };
        (
            Callbacks::from(ud),
            kind,
            bytes(&content).to_vec(),
            truncated,
        )
    };
    let truncation = if truncated {
        Truncation::Truncated
    } else {
        Truncation::Complete
    };
    cb.send(VtEvent::Unknown {
        kind,
        payload,
        truncation,
    });
}

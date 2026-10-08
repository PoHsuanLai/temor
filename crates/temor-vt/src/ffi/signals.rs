//! The plain callbacks: replies, bell, title, working directory, reset, clipboard, render holds,
//! colour scheme and size queries, and unknown sequences.

use std::ffi::c_void;

use temor_vt_sys as sys;

use super::callbacks::{TitleChange, guarded};
use super::convert::{bytes, string};
use crate::{
    ClipboardGrant, ClipboardItem, ClipboardLocation, ClipboardWrite, ColorScheme, RenderHold,
    Truncation, UnknownKind, VtEvent,
};

pub(super) unsafe extern "C" fn on_write_pty(
    _t: sys::Terminal,
    ud: *mut c_void,
    data: *const u8,
    len: usize,
) {
    if data.is_null() || len == 0 {
        return;
    }
    // SAFETY: libghostty passes `len` readable bytes valid for this call; `ud` is ours.
    unsafe {
        guarded(ud, (), |cb| {
            cb.replies.reply(std::slice::from_raw_parts(data, len))
        })
    }
}

pub(super) unsafe extern "C" fn on_bell(_t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: `ud` is ours.
    unsafe { guarded(ud, (), |cb| cb.send(VtEvent::Bell)) }
}

pub(super) unsafe extern "C" fn on_title(_t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: `ud` is ours. The title is read after `vt_write` returns, as libghostty requires.
    unsafe { guarded(ud, (), |cb| cb.title.set(TitleChange::Changed)) }
}

pub(super) unsafe extern "C" fn on_pwd(t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: libghostty allows reading PWD inside this callback; the string is borrowed until
    // the next mutating call and copied before returning. `ud` is ours.
    unsafe {
        guarded(ud, (), |cb| {
            let mut pwd = sys::String {
                ptr: std::ptr::null(),
                len: 0,
            };
            sys::ghostty_terminal_get(t, sys::TerminalData::PWD, (&raw mut pwd).cast());
            cb.send(VtEvent::Cwd(string(pwd)));
        })
    }
}

pub(super) unsafe extern "C" fn on_reset(_t: sys::Terminal, ud: *mut c_void) {
    // SAFETY: `ud` is ours. A reset clears the title without TITLE_CHANGED, so a title change
    // seen earlier in this write is stale.
    unsafe {
        guarded(ud, (), |cb| {
            cb.title.set(TitleChange::Unchanged);
            cb.send(VtEvent::Reset);
        })
    }
}

pub(super) unsafe extern "C" fn on_clipboard(
    _t: sys::Terminal,
    ud: *mut c_void,
    w: *const sys::ClipboardWrite,
) {
    if w.is_null() {
        return;
    }
    // SAFETY: `w` and everything it points to are valid for this call; `contents` holds
    // `contents_len` entries; strings are copied here. `ud` is ours. `reply` is the function
    // libghostty gave for this request, called within it.
    unsafe {
        guarded(ud, (), |cb| {
            let w = &*w;
            let items = if w.contents.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(w.contents, w.contents_len)
                    .iter()
                    .map(|c| ClipboardItem {
                        mime: string(c.mime),
                        data: bytes(&c.data).to_vec(),
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
            let program = string(w.name);
            cb.send(VtEvent::Clipboard {
                write: ClipboardWrite {
                    location,
                    items,
                    program,
                    grant,
                },
            });
            // Not replying denies the write; the host decides later, so the program is told it
            // worked.
            let mut reply = temor_vt_sys::sized!(sys::ClipboardWriteReply);
            reply.result = sys::ClipboardWriteResult::SUCCESS;
            if let Some(answer) = w.reply {
                answer(w, &reply);
            }
        })
    }
}

pub(super) unsafe extern "C" fn on_render_hold(_t: sys::Terminal, ud: *mut c_void, held: bool) {
    let hold = if held {
        RenderHold::Held
    } else {
        RenderHold::Released
    };
    // SAFETY: `ud` is ours.
    unsafe { guarded(ud, (), |cb| cb.send(VtEvent::RenderHold(hold))) }
}

pub(super) unsafe extern "C" fn on_color_scheme(
    _t: sys::Terminal,
    ud: *mut c_void,
    out: *mut sys::ColorScheme::Type,
) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY: `ud` is ours; `out` is a valid out-pointer for this call.
    unsafe {
        guarded(ud, false, |cb| {
            out.write(match cb.color_scheme.get() {
                ColorScheme::Light => sys::ColorScheme::LIGHT,
                ColorScheme::Dark => sys::ColorScheme::DARK,
            });
            cb.send(VtEvent::ColorScheme);
            true
        })
    }
}

pub(super) unsafe extern "C" fn on_size(
    _t: sys::Terminal,
    ud: *mut c_void,
    out: *mut sys::SizeReportSize,
) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY: `ud` is ours; `out` is a valid out-pointer for this call.
    unsafe {
        guarded(ud, false, |cb| {
            let (size, cell) = cb.size.get();
            out.write(sys::SizeReportSize {
                rows: size.rows.0,
                columns: size.cols.0,
                cell_width: cell.width.0,
                cell_height: cell.height.0,
            });
            true
        })
    }
}

pub(super) unsafe extern "C" fn on_unknown(
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
    unsafe {
        guarded(ud, (), |cb| {
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
            let truncation = if truncated {
                Truncation::Truncated
            } else {
                Truncation::Complete
            };
            cb.send(VtEvent::Unknown {
                kind,
                payload: bytes(&content).to_vec(),
                truncation,
            });
        })
    }
}

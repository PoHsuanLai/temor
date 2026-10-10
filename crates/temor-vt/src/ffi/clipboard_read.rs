//! The clipboard read callback (OSC 52 with `?`): answered from the terminal's
//! [`ClipboardSource`], never by waiting for the window.

use std::ffi::c_void;

use temor_vt_sys as sys;

use super::callbacks::guarded;
use super::convert::string;
use crate::{ClipboardAnswer, ClipboardLocation, ClipboardRead, VtEvent};

/// What the callback tells libghostty.
enum Reply {
    Text(String),
    Empty,
    Denied,
}

pub(super) unsafe extern "C" fn on_clipboard_read(
    _t: sys::Terminal,
    ud: *mut c_void,
    r: *const sys::ClipboardRead,
) {
    if r.is_null() {
        return;
    }
    // SAFETY: `r` and what it points to are valid for this call; strings are copied here. `ud`
    // is ours. `reply` is the function libghostty gave for this request, called within it.
    unsafe {
        guarded(ud, (), |cb| {
            let r = &*r;
            let read = ClipboardRead {
                location: match r.location {
                    sys::ClipboardLocation::SELECTION => ClipboardLocation::Selection,
                    sys::ClipboardLocation::PRIMARY => ClipboardLocation::Primary,
                    _ => ClipboardLocation::Standard,
                },
                program: string(r.name),
            };
            let answer = match cb.clipboard.borrow().as_ref() {
                Some(source) => source.read(&read),
                None => ClipboardAnswer::Denied,
            };
            let reply = match answer {
                ClipboardAnswer::Text(text) => Reply::Text(text),
                ClipboardAnswer::Empty => Reply::Empty,
                ClipboardAnswer::Ask => {
                    cb.send(VtEvent::ClipboardRead { read });
                    Reply::Denied
                }
                _ => Reply::Denied,
            };
            answer_with(r, &reply);
        })
    }
}

/// Calls the request's reply function with `reply`.
///
/// # Safety
/// `r` must be the live request libghostty passed to the callback now running.
unsafe fn answer_with(r: &sys::ClipboardRead, reply: &Reply) {
    let Some(answer) = r.reply else {
        return;
    };
    const MIME: &[u8] = b"text/plain";
    let mut out = temor_vt_sys::sized!(sys::ClipboardReadReply);
    let content;
    match reply {
        Reply::Text(text) => {
            content = sys::ClipboardContent {
                mime: sys::String {
                    ptr: MIME.as_ptr(),
                    len: MIME.len(),
                },
                data: sys::String {
                    ptr: text.as_ptr(),
                    len: text.len(),
                },
            };
            out.result = sys::ClipboardReadResult::SUCCESS;
            out.contents = &raw const content;
            out.contents_len = 1;
        }
        Reply::Empty => out.result = sys::ClipboardReadResult::SUCCESS,
        Reply::Denied => out.result = sys::ClipboardReadResult::DENIED,
    }
    // SAFETY: `r` is live per this function's contract; `out`, `content` and `text` outlive the
    // call, and libghostty copies what it needs before returning.
    unsafe { answer(r, &out) };
}

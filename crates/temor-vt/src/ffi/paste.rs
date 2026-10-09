//! libghostty's paste safety check (`input/paste.zig`): text that contains a newline, or the end
//! of a bracketed paste, can run commands once pasted.

use temor_vt_sys as sys;

/// Whether libghostty considers `text` safe to paste as it is. Carriage returns are not checked
/// by libghostty; the app treats them as line breaks.
pub fn paste_is_safe(text: &[u8]) -> bool {
    // SAFETY: the pointer and length describe `text`, which lives for the call; libghostty only
    // reads it and keeps no reference.
    unsafe { sys::ghostty_paste_is_safe(text.as_ptr().cast(), text.len()) }
}

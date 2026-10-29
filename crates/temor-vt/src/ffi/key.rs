//! `KeyEncoder`: [`KeyInput`] to the bytes a program expects (legacy, xterm or kitty protocol).

use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::convert::check;
use super::key_code::ghostty_key;
use super::terminal::Terminal;
use crate::{KeyAction, KeyInput, KittyKeyFlags, Modifiers, OptionAsAlt, VtError};

/// Encodes key events. Call [`KeyEncoder::sync`] with the terminal before encoding so the
/// program's modes (cursor keys, keypad, kitty flags) apply.
pub struct KeyEncoder {
    encoder: NonNull<sys::KeyEncoderImpl>,
    event: NonNull<sys::KeyEventImpl>,
    option_as_alt: OptionAsAlt,
}

// SAFETY: the encoder and event are plain libghostty memory owned by this value, with no
// callbacks or thread-local state; `&mut self` serialises every call.
unsafe impl Send for KeyEncoder {}

impl std::fmt::Debug for KeyEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyEncoder").finish_non_exhaustive()
    }
}

/// The modifiers the layout used up to make the key's text: Shift, for the capital or the
/// symbol it typed. Without it a program that asked for the kitty keyboard protocol's
/// disambiguation alone (`CSI > 1 u`) is sent `CSI 97 ; 2 u` for Shift+A instead of `A`: the
/// encoder sends text as text only when the modifiers left after these are none.
fn consumed_mods(key: &KeyInput) -> u16 {
    let typed = key.text.chars().next().is_some_and(|c| !c.is_control());
    match typed && key.mods.contains(Modifiers::SHIFT) {
        true => Modifiers::SHIFT.bits(),
        false => 0,
    }
}

impl KeyEncoder {
    /// An encoder with default (legacy) settings.
    pub fn new() -> Result<Self, VtError> {
        let (mut encoder, mut event) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: valid out-pointers; a null allocator is the default one.
        let made = unsafe {
            check(
                sys::ghostty_key_encoder_new(std::ptr::null(), &mut encoder),
                "key encoder",
            )
            .and_then(|()| {
                check(
                    sys::ghostty_key_event_new(std::ptr::null(), &mut event),
                    "key event",
                )
            })
        };
        match (made, NonNull::new(encoder), NonNull::new(event)) {
            (Ok(()), Some(encoder), Some(event)) => Ok(Self {
                encoder,
                event,
                option_as_alt: OptionAsAlt::default(),
            }),
            (made, ..) => {
                // SAFETY: both frees accept null.
                unsafe {
                    sys::ghostty_key_event_free(event);
                    sys::ghostty_key_encoder_free(encoder);
                }
                Err(made.err().unwrap_or(VtError::OutOfMemory))
            }
        }
    }

    /// Takes the terminal's current key modes (cursor keys, keypad, kitty flags, ...). The
    /// Option-as-Alt choice is the caller's, not the terminal's, so it is put back afterwards.
    pub fn sync(&mut self, term: &Terminal) {
        // SAFETY: both handles are live.
        unsafe { sys::ghostty_key_encoder_setopt_from_terminal(self.encoder.as_ptr(), term.raw()) };
        self.apply_option_as_alt();
    }

    /// Whether macOS's Option key acts as Alt. It holds across [`KeyEncoder::sync`]. Systems
    /// without an Option key ignore it.
    pub fn set_option_as_alt(&mut self, option: OptionAsAlt) {
        self.option_as_alt = option;
        self.apply_option_as_alt();
    }

    fn apply_option_as_alt(&mut self) {
        let value: sys::OptionAsAlt::Type = match self.option_as_alt {
            OptionAsAlt::Off => sys::OptionAsAlt::FALSE,
            OptionAsAlt::Both => sys::OptionAsAlt::TRUE,
            OptionAsAlt::Left => sys::OptionAsAlt::LEFT,
            OptionAsAlt::Right => sys::OptionAsAlt::RIGHT,
        };
        // SAFETY: MACOS_OPTION_AS_ALT takes a `GhosttyOptionAsAlt*` (an `int`).
        unsafe {
            sys::ghostty_key_encoder_setopt(
                self.encoder.as_ptr(),
                sys::KeyEncoderOption::MACOS_OPTION_AS_ALT,
                (&raw const value).cast(),
            )
        };
    }

    /// Overrides the kitty keyboard flags.
    pub fn set_kitty_flags(&mut self, flags: KittyKeyFlags) {
        let bits = flags.bits();
        // SAFETY: KITTY_FLAGS takes a `GhosttyKittyKeyFlags*` (`uint8_t*`).
        unsafe {
            sys::ghostty_key_encoder_setopt(
                self.encoder.as_ptr(),
                sys::KeyEncoderOption::KITTY_FLAGS,
                (&raw const bits).cast(),
            )
        };
    }

    /// The bytes for `key`; empty when the key sends nothing (a bare modifier, say).
    pub fn encode(&mut self, key: &KeyInput) -> Result<Vec<u8>, VtError> {
        let ev = self.event.as_ptr();
        let action = match key.action {
            KeyAction::Press => sys::KeyAction::PRESS,
            KeyAction::Repeat => sys::KeyAction::REPEAT,
            KeyAction::Release => sys::KeyAction::RELEASE,
        };
        let text = key.text.as_bytes();
        // SAFETY: the event is live. It borrows `text` without copying; `key` outlives the
        // encode calls below, and the text is cleared before returning.
        unsafe {
            sys::ghostty_key_event_set_action(ev, action);
            sys::ghostty_key_event_set_key(ev, ghostty_key(key.code));
            sys::ghostty_key_event_set_mods(ev, key.mods.bits());
            sys::ghostty_key_event_set_consumed_mods(ev, consumed_mods(key));
            sys::ghostty_key_event_set_composing(ev, key.composing);
            sys::ghostty_key_event_set_unshifted_codepoint(ev, key.unshifted.map_or(0, u32::from));
            if text.is_empty() {
                sys::ghostty_key_event_set_utf8(ev, std::ptr::null(), 0);
            } else {
                sys::ghostty_key_event_set_utf8(ev, text.as_ptr().cast(), text.len());
            }
        }
        let out = self.encode_event();
        // SAFETY: drops the event's borrow of `text`.
        unsafe { sys::ghostty_key_event_set_utf8(ev, std::ptr::null(), 0) };
        out
    }

    fn encode_event(&mut self) -> Result<Vec<u8>, VtError> {
        let mut buf = vec![0u8; 64];
        loop {
            let mut len = 0usize;
            // SAFETY: `buf` has `buf.len()` writable bytes; encoder and event are live.
            let code = unsafe {
                sys::ghostty_key_encoder_encode(
                    self.encoder.as_ptr(),
                    self.event.as_ptr(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    &mut len,
                )
            };
            if code == sys::Result::OUT_OF_SPACE && len > buf.len() {
                buf.resize(len, 0);
                continue;
            }
            check(code, "key")?;
            buf.truncate(len);
            return Ok(buf);
        }
    }
}

impl Drop for KeyEncoder {
    fn drop(&mut self) {
        // SAFETY: made in `new`, freed once.
        unsafe {
            sys::ghostty_key_event_free(self.event.as_ptr());
            sys::ghostty_key_encoder_free(self.encoder.as_ptr());
        }
    }
}

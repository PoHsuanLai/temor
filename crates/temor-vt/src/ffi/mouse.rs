//! `MouseEncoder`: [`MouseInput`] to the bytes a mouse-tracking program expects.

use std::ptr::NonNull;

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{MouseAction, MouseButton, MouseFormat, MouseInput, MouseSurface, VtError};

/// Encodes mouse events. Call [`MouseEncoder::sync`] after the program changes mouse modes and
/// [`MouseEncoder::set_surface`] after a resize.
pub struct MouseEncoder {
    encoder: NonNull<sys::MouseEncoderImpl>,
    event: NonNull<sys::MouseEventImpl>,
}

// SAFETY: the encoder and event are plain libghostty memory owned by this value, with no
// callbacks or thread-local state; `&mut self` serialises every call.
unsafe impl Send for MouseEncoder {}

impl std::fmt::Debug for MouseEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MouseEncoder").finish_non_exhaustive()
    }
}

impl MouseEncoder {
    /// An encoder that reports nothing until synced with a terminal that tracks the mouse.
    pub fn new() -> Result<Self, VtError> {
        let (mut encoder, mut event) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: valid out-pointers; a null allocator is the default one.
        let made = unsafe {
            check(
                sys::ghostty_mouse_encoder_new(std::ptr::null(), &mut encoder),
                "mouse encoder",
            )
            .and_then(|()| {
                check(
                    sys::ghostty_mouse_event_new(std::ptr::null(), &mut event),
                    "mouse event",
                )
            })
        };
        match (made, NonNull::new(encoder), NonNull::new(event)) {
            (Ok(()), Some(encoder), Some(event)) => Ok(Self { encoder, event }),
            (made, ..) => {
                // SAFETY: both frees accept null.
                unsafe {
                    sys::ghostty_mouse_event_free(event);
                    sys::ghostty_mouse_encoder_free(encoder);
                }
                Err(made.err().unwrap_or(VtError::OutOfMemory))
            }
        }
    }

    /// Takes the terminal's mouse tracking mode and format.
    pub fn sync(&mut self, term: &Terminal) {
        // SAFETY: both handles are live.
        unsafe {
            sys::ghostty_mouse_encoder_setopt_from_terminal(self.encoder.as_ptr(), term.raw())
        };
    }

    /// Forces a report format; `Auto` leaves the one `sync` took.
    pub fn set_format(&mut self, format: MouseFormat) {
        let value = match format {
            MouseFormat::Auto => return,
            MouseFormat::X10 => sys::MouseFormat::X10,
            MouseFormat::Utf8 => sys::MouseFormat::UTF8,
            MouseFormat::Sgr => sys::MouseFormat::SGR,
            MouseFormat::Urxvt => sys::MouseFormat::URXVT,
            MouseFormat::SgrPixels => sys::MouseFormat::SGR_PIXELS,
        };
        // SAFETY: FORMAT takes a `GhosttyMouseFormat*`.
        unsafe {
            sys::ghostty_mouse_encoder_setopt(
                self.encoder.as_ptr(),
                sys::MouseEncoderOption::FORMAT,
                (&raw const value).cast(),
            )
        };
    }

    /// The surface geometry pixels are mapped to cells with.
    pub fn set_surface(&mut self, s: MouseSurface) {
        let mut size = temor_vt_sys::sized!(sys::MouseEncoderSize);
        size.screen_width = s.width.0;
        size.screen_height = s.height.0;
        size.cell_width = s.cell_width.0;
        size.cell_height = s.cell_height.0;
        let [top, right, bottom, left] = s.padding;
        (
            size.padding_top,
            size.padding_right,
            size.padding_bottom,
            size.padding_left,
        ) = (top.0, right.0, bottom.0, left.0);
        // SAFETY: SIZE takes a sized `GhosttyMouseEncoderSize*`.
        unsafe {
            sys::ghostty_mouse_encoder_setopt(
                self.encoder.as_ptr(),
                sys::MouseEncoderOption::SIZE,
                (&raw const size).cast(),
            )
        };
    }

    /// The bytes for `input`; empty when the program does not track this kind of event.
    pub fn encode(&mut self, input: &MouseInput) -> Result<Vec<u8>, VtError> {
        let ev = self.event.as_ptr();
        let action = match input.action {
            MouseAction::Press => sys::MouseAction::PRESS,
            MouseAction::Release => sys::MouseAction::RELEASE,
            MouseAction::Motion => sys::MouseAction::MOTION,
        };
        // Positions are whole pixels, so the f32 conversion is exact well past any screen size.
        let position = sys::MousePosition {
            x: input.at.x as f32,
            y: input.at.y as f32,
        };
        // SAFETY: the event is live.
        unsafe {
            sys::ghostty_mouse_event_set_action(ev, action);
            match input.button {
                Some(b) => sys::ghostty_mouse_event_set_button(ev, button(b)),
                None => sys::ghostty_mouse_event_clear_button(ev),
            }
            sys::ghostty_mouse_event_set_mods(ev, input.mods.bits());
            sys::ghostty_mouse_event_set_position(ev, position);
        }
        let mut buf = vec![0u8; 64];
        loop {
            let mut len = 0usize;
            // SAFETY: `buf` has `buf.len()` writable bytes; encoder and event are live.
            let code = unsafe {
                sys::ghostty_mouse_encoder_encode(
                    self.encoder.as_ptr(),
                    ev,
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    &mut len,
                )
            };
            if code == sys::Result::OUT_OF_SPACE && len > buf.len() {
                buf.resize(len, 0);
                continue;
            }
            check(code, "mouse")?;
            buf.truncate(len);
            return Ok(buf);
        }
    }
}

fn button(b: MouseButton) -> sys::MouseButton::Type {
    match b {
        MouseButton::Left => sys::MouseButton::LEFT,
        MouseButton::Middle => sys::MouseButton::MIDDLE,
        MouseButton::Right => sys::MouseButton::RIGHT,
        MouseButton::WheelUp => sys::MouseButton::FOUR,
        MouseButton::WheelDown => sys::MouseButton::FIVE,
        MouseButton::WheelLeft => sys::MouseButton::SIX,
        MouseButton::WheelRight => sys::MouseButton::SEVEN,
        MouseButton::Back => sys::MouseButton::EIGHT,
        MouseButton::Forward => sys::MouseButton::NINE,
    }
}

impl Drop for MouseEncoder {
    fn drop(&mut self) {
        // SAFETY: made in `new`, freed once.
        unsafe {
            sys::ghostty_mouse_event_free(self.event.as_ptr());
            sys::ghostty_mouse_encoder_free(self.encoder.as_ptr());
        }
    }
}

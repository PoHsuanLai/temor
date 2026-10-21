//! OS-specific edges. This is the only module allowed to name a target OS: everything else
//! reaches the platform through the small traits here.

mod edge;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod other;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod strikes;

pub(crate) use edge::{BitmapEmojiSource, ColorBitmap};
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) use other::SystemBitmapEmoji;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use strikes::SystemBitmapEmoji;

/// Families tried by name before the system's `monospace` generic. On a Mac that generic is
/// Courier, which is thin and cramped on a screen; Menlo and SF Mono are what the system's own
/// terminals use.
#[cfg(target_os = "macos")]
pub(crate) const MONOSPACE_FIRST: &[&str] = &["SF Mono", "Menlo"];
#[cfg(not(target_os = "macos"))]
pub(crate) const MONOSPACE_FIRST: &[&str] = &[];

/// The least a line is tall, in ems: Menlo's own line is 1.17, which reads tight under the UI's
/// type. Zero where the font's own metrics are kept.
#[cfg(target_os = "macos")]
pub(crate) const MIN_LINE_EM: f32 = 1.25;
#[cfg(not(target_os = "macos"))]
pub(crate) const MIN_LINE_EM: f32 = 0.0;

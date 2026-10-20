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

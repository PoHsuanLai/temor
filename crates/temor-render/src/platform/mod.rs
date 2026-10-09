//! OS-specific edges. This is the only module allowed to name a target OS: everything else
//! reaches the platform through the small traits here.

mod edge;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod other;

pub(crate) use edge::{BitmapEmojiSource, ColorBitmap};
#[cfg(target_os = "linux")]
pub(crate) use linux::SystemBitmapEmoji;
#[cfg(not(target_os = "linux"))]
pub(crate) use other::SystemBitmapEmoji;

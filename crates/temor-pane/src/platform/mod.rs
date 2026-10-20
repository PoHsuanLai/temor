//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: the peer's user id from `SO_PEERCRED`.
//! - macOS: the same from `LOCAL_PEERCRED`.
//! - Other systems: stubs that refuse. TODO(platform:windows) marks where the owner's machine
//!   takes over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{own_uid, peer_uid};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{own_uid, peer_uid};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) use unsupported::{own_uid, peer_uid};

/// Whether the Option key is Alt: on macOS both Option keys send the escape prefix, as most
/// terminals' people expect (Option+B moves back a word); no other system has the key.
pub(crate) const OPTION_AS_ALT: temor_vt::OptionAsAlt = if cfg!(target_os = "macos") {
    temor_vt::OptionAsAlt::Both
} else {
    temor_vt::OptionAsAlt::Off
};

//! temor-vt conformance: recorded byte streams in `tests/fixtures/*.bin`, each module pairing its
//! fixtures with the outcomes libghostty must produce. A Ghostty pin bump must pass these.

mod fake;

#[cfg(feature = "ghostty")]
mod support;

#[cfg(feature = "ghostty")]
mod graphemes;
#[cfg(feature = "ghostty")]
mod inline_file;
#[cfg(feature = "ghostty")]
mod keys;
#[cfg(feature = "ghostty")]
mod kitty;
#[cfg(feature = "ghostty")]
mod mouse;
#[cfg(feature = "ghostty")]
mod replies;
#[cfg(feature = "ghostty")]
mod screen;
#[cfg(feature = "ghostty")]
mod sgr;
#[cfg(feature = "ghostty")]
mod shell;
#[cfg(feature = "ghostty")]
mod snapshot;
#[cfg(feature = "ghostty")]
mod threads;

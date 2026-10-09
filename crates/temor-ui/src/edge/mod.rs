//! The app's reads from outside: the process environment, the settings file, the desktop's
//! notifications and the clock. Everything here is read once, at start, and handed to the rest
//! of the app as values or trait objects.

mod ambient;
pub mod clock;
pub mod notify;
mod save_pace;
mod session_file;
mod settings_file;
pub mod sidecar;

pub use ambient::Ambient;
pub use clock::{Clock, FakeClock, SystemClock};
pub use notify::{DesktopNote, NoopNotifier, Notifier, NotifyFailed, RecordingNotifier};
pub use save_pace::{Pace, SavePace};
pub use session_file::{
    FILE_NAME, Startup, load as load_session, resolve_state_dir, save as save_session,
};
pub use settings_file::load_settings;

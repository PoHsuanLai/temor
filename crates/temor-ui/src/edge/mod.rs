//! The app's reads from outside: the process environment, the settings file, the desktop's
//! notifications and the clock. Everything here is read once, at start, and handed to the rest
//! of the app as values or trait objects.

mod ambient;
pub mod clock;
pub mod desktop_feed;
pub mod desktop_port;
pub mod git;
mod keymap;
pub mod notify;
mod save_pace;
mod session_file;
mod settings_file;
pub mod shelf_files;
pub mod sidecar;
pub mod space_agent;
pub mod space_start;
mod spaces_file;
mod unix_now;

pub use ambient::Ambient;
pub use clock::{Clock, FakeClock, SystemClock};
pub use keymap::{keymap_for, load_keymap};
pub use notify::{DesktopNote, NoopNotifier, Notifier, NotifyFailed, RecordingNotifier};
pub use save_pace::{Pace, SavePace};
pub use session_file::{
    FILE_NAME, Startup, file_name as session_file_name, load as load_session,
    load_for as load_session_for, remove_for as remove_session_for, resolve_state_dir,
    save as save_session, save_for as save_session_for,
};
pub use settings_file::load_settings;
pub use space_start::{CredentialFault, CredentialSource, Credentials, NoCredentials};
pub use spaces_file::{SPACES_FILE, SpacesStartup, load_spaces, save_spaces};
pub use unix_now::unix_now;

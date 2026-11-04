//! The app's reads from outside: the process environment, the settings file, the desktop's
//! notifications and the clock. Everything here is read once, at start, and handed to the rest
//! of the app as values or trait objects.

mod ambient;
pub mod clock;
mod keymap;
mod keymap_source;
pub mod notify;
pub mod offer;
pub mod peek_look;
pub mod peek_mail;
mod settings_file;
mod unix_now;
pub mod viewer_workers;

pub use ambient::Ambient;
pub use clock::{Clock, FakeClock, SystemClock};
pub use keymap::{keymap_for, load_keymap};
pub use keymap_source::{LOOK_EVERY, system_keys, watch_file};
pub use notify::{DesktopNote, NoopNotifier, Notifier, NotifyFailed, RecordingNotifier};
pub use settings_file::{ThemeChoice, read_theme_choice, theme_choice};
pub use temor_client::space::start::{
    CredentialFault, CredentialSource, Credentials, NoCredentials,
};
pub use temor_client::space::vault::VaultCredentials;
pub use temor_panes::report_socket::SocketHost;
pub use unix_now::unix_now;

//! The app's reads from outside: the process environment, the settings file, the desktop's
//! notifications and the clock. Everything here is read once, at start, and handed to the rest
//! of the app as values or trait objects.

mod agent_socket;
mod ambient;
pub mod app_icons;
pub mod clock;
pub mod desktop_feed;
pub mod desktop_port;
pub mod facts;
mod facts_files;
pub mod facts_host;
pub mod folder_read;
pub mod icon_pack;
mod keymap;
mod keymap_source;
pub mod local_zone;
pub mod notify;
pub mod offer;
pub mod peek_look;
pub mod peek_mail;
mod settings_file;
pub mod shelf_files;
pub mod space_agent;
pub mod space_start;
pub mod space_vault;
mod unix_now;
pub mod viewer_workers;

pub use agent_socket::SocketHost;
pub use ambient::Ambient;
pub use clock::{Clock, FakeClock, SystemClock};
pub use keymap::{keymap_for, load_keymap};
pub use keymap_source::{LOOK_EVERY, system_keys, watch_file};
pub use notify::{DesktopNote, NoopNotifier, Notifier, NotifyFailed, RecordingNotifier};
pub use settings_file::{ThemeChoice, read_theme_choice, theme_choice};
pub use space_start::{CredentialFault, CredentialSource, Credentials, Held, NoCredentials};
pub use space_vault::VaultCredentials;
pub use unix_now::unix_now;

//! The app's reads from outside: the process environment, the settings file, the desktop's
//! notifications and the clock. Everything here is read once, at start, and handed to the rest
//! of the app as values or trait objects.

mod agent_socket;
mod ambient;
pub mod app_icons;
pub mod checkpoint;
pub mod clock;
pub mod desktop_feed;
pub mod desktop_port;
pub mod facts;
mod facts_files;
pub mod facts_host;
pub mod icon_pack;
mod keymap;
mod keymap_source;
pub mod local_zone;
pub mod notify;
pub mod offer;
pub mod peek_look;
pub mod peek_mail;
pub mod remote;
pub mod remote_adopt;
pub mod remote_attach;
pub mod remote_builds;
pub mod remote_calls;
pub mod remote_conn;
pub mod remote_edit;
pub mod remote_edit_stream;
pub mod remote_event;
pub mod remote_grants;
pub mod remote_hold;
pub mod remote_install;
pub mod remote_link;
pub mod remote_palette;
pub mod remote_ping;
pub mod remote_pump;
pub mod remote_relay;
pub mod remote_route;
pub mod replay;
mod save_pace;
mod session_file;
mod settings_file;
pub mod shelf_files;
pub mod sidecar;
pub mod space_agent;
pub mod space_start;
pub mod space_vault;
mod spaces_file;
mod ssh_env;
mod ssh_hosts_cache;
pub mod ssh_work;
#[cfg(test)]
mod ssh_work_tests;
mod unix_now;
pub mod upload_work;
pub mod viewer_host;
pub mod viewer_workers;

pub use agent_socket::SocketHost;
pub use ambient::Ambient;
pub use clock::{Clock, FakeClock, SystemClock};
pub use keymap::{keymap_for, load_keymap};
pub use keymap_source::{LOOK_EVERY, system_keys, watch_file};
pub use notify::{DesktopNote, NoopNotifier, Notifier, NotifyFailed, RecordingNotifier};
pub use save_pace::{Pace, SavePace};
pub use session_file::{
    FILE_NAME, Startup, file_name as session_file_name, load as load_session,
    load_for as load_session_for, load_hosts, remove_for as remove_session_for, resolve_state_dir,
    save as save_session, save_for as save_session_for, save_with_hosts,
};
pub use settings_file::{ThemeChoice, read_theme_choice, theme_choice};
pub use space_start::{CredentialFault, CredentialSource, Credentials, Held, NoCredentials};
pub use space_vault::VaultCredentials;
pub use spaces_file::{SPACES_FILE, SpacesStartup, load_spaces, save_spaces};
pub use ssh_env::{ArcRunner, SshEnv};
pub use unix_now::unix_now;

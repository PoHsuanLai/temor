//! The pictures a tab's row leads with (PLAN P22), found off the window's thread: the app a
//! program belongs to and its icon, a folder's own icon, and the icon pack's brands. Which one a
//! row shows is temor-core's (`icon::icon_for`); this finds them. A look-up runs once on the
//! `temor-icons` thread; until it is done the row shows what it would without it, and the window
//! is told when it is, to draw the row again.

mod apps;
mod folders;
mod pack;

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use parking_lot::Mutex;

pub use apps::{AppIcon, app_png};
pub use pack::IconPack;

/// A picture to draw as it is: an app's icon, or a folder's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Image {
    Png(Arc<[u8]>),
    Svg(Arc<str>),
}

/// What is looked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Look {
    /// The app of the program `name`, found along `path`.
    App { name: String, path: OsString },
    /// The icon set on a folder.
    Folder(PathBuf),
}

/// What a look-up came to, or that it is under way.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Looking,
    App(Option<AppIcon>),
    Folder(Option<Image>),
}

/// The window's look-ups: each done once, on a thread of its own, the window told when one is.
pub struct Icons {
    seen: Arc<Mutex<HashMap<Look, Seen>>>,
    /// The way to the look-up thread; none looks up on the asking thread (a test's).
    asks: Option<Sender<Look>>,
}

impl std::fmt::Debug for Icons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icons")
            .field("seen", &self.seen.lock().len())
            .finish_non_exhaustive()
    }
}

impl Icons {
    /// Look-ups on the `temor-icons` thread, which calls `told` after each one is done. The
    /// thread ends with the `Icons`. If it cannot start, look-ups run on the asking thread.
    pub fn new(told: impl Fn() + Send + 'static) -> Icons {
        let seen: Arc<Mutex<HashMap<Look, Seen>>> = Arc::default();
        let (asks, looks) = channel::<Look>();
        let found = Arc::clone(&seen);
        let started = std::thread::Builder::new()
            .name("temor-icons".to_owned())
            .spawn(move || {
                for look in looks {
                    let answer = look_up(&look);
                    found.lock().insert(look, answer);
                    told();
                }
            });
        Icons {
            seen,
            asks: started.is_ok().then_some(asks),
        }
    }

    /// Look-ups done at once on the asking thread.
    pub fn inline() -> Icons {
        Icons {
            seen: Arc::default(),
            asks: None,
        }
    }

    /// The app the program `name` belongs to and its icon, found along `path` (the shell's
    /// `PATH`); none for a program in no app, and while the look-up runs.
    pub fn app(&self, name: &str, path: &OsStr) -> Option<AppIcon> {
        let look = Look::App {
            name: name.to_owned(),
            path: path.to_owned(),
        };
        match self.ask(look)? {
            Seen::App(found) => found,
            _ => None,
        }
    }

    /// The icon set on the folder `dir`; none for a folder without one, and while the look-up
    /// runs.
    pub fn folder(&self, dir: &Path) -> Option<Image> {
        match self.ask(Look::Folder(dir.to_owned()))? {
            Seen::Folder(found) => found,
            _ => None,
        }
    }

    /// What `look` came to: at once when it was done before or is done inline, else none while
    /// the thread looks.
    fn ask(&self, look: Look) -> Option<Seen> {
        if let Some(seen) = self.seen.lock().get(&look) {
            return Some(seen.clone());
        }
        let Some(asks) = &self.asks else {
            let answer = look_up(&look);
            self.seen.lock().insert(look, answer.clone());
            return Some(answer);
        };
        self.seen.lock().insert(look.clone(), Seen::Looking);
        if asks.send(look.clone()).is_err() {
            // The thread is gone: ask again next time.
            self.seen.lock().remove(&look);
        }
        None
    }
}

fn look_up(look: &Look) -> Seen {
    match look {
        Look::App { name, path } => Seen::App(apps::app_of(name, path)),
        Look::Folder(dir) => Seen::Folder(folders::folder_icon(dir)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    #[test]
    fn a_look_up_runs_once_on_its_thread_and_the_window_is_told_when_it_is_done() {
        let (told, heard) = channel();
        let icons = Icons::new(move || {
            let _ = told.send(());
        });
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(icons.folder(dir.path()), None, "looking");
        assert_eq!(heard.recv_timeout(Duration::from_secs(10)), Ok(()));
        assert_eq!(icons.folder(dir.path()), None, "a folder without an icon");
        assert_eq!(
            heard.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout),
            "looked up once"
        );
        assert!(matches!(
            icons.seen.lock().get(&Look::Folder(dir.path().to_owned())),
            Some(Seen::Folder(None))
        ));
    }

    #[test]
    fn inline_look_ups_answer_at_once() {
        let icons = Icons::inline();
        assert_eq!(icons.app("no-such-program-here", OsStr::new("/bin")), None);
        assert_eq!(icons.seen.lock().len(), 1);
    }
}

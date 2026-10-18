//! A hub whose `ssh` and `scp` are stand-ins, and a way to let its jobs finish.

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use temor_pane::text::LinkTarget;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::edge::Ambient;
use crate::edge::remote_event::RemoteEvent;
use crate::edge::ssh_work::SshEvent;
use crate::hub::model::Hub;
use crate::hub::ssh_view::{HostView, Stage};
use crate::platform::{FileShell, OpenError, Opener};
use crate::view::launch::{Launch, LauncherChoice};
use crate::view::theme::AppearanceSource;

/// The stand-in `ssh`. `-O check` says a connection is up when `up` exists; `-O forward` and
/// `-O cancel` exit with the code in `control-exit`; `-G` resolves any alias to `resolved.example.com` port 2200; `-T` runs the remote command here (the file
/// calls); anything else is a session: its arguments and the decoded setup are kept in
/// `session-N` and `session-N.script`, `banner` is printed, and it stays until the test ends.
/// The connecting `ssh` (it runs `true`) asks for a password: the right one (`hunter2`) makes the
/// connection (`up` appears, the password is kept in `connect-ok`) and it exits well; any other
/// ends it with 255.
const FAKE_SSH: &str = r#"#!/bin/sh
d=@DIR@
for last; do :; done
case "$1" in
  -G) printf 'hostname resolved.example.com\nport 2200\n'; exit 0 ;;
  -O)
    case "$2" in
      check) [ -e "$d/up" ] && exit 0; exit 255 ;;
      *) printf '%s\n' "$@" >> "$d/control.log"; exit "$(cat "$d/control-exit" 2>/dev/null || echo 0)" ;;
    esac ;;
  -T) exec sh -c "$last" ;;
esac
if [ "$last" = true ]; then
  printf 'Password: '
  read pw
  [ "$pw" = hunter2 ] || exit 255
  echo "$pw" > "$d/connect-ok"
  : > "$d/up"
  exit 0
fi
n=$(($(cat "$d/count" 2>/dev/null || echo 0) + 1))
echo "$n" > "$d/count"
printf '%s\n' "$@" > "$d/session-$n"
case "$last" in
  "sh -c "*) printf %s "$last" | sed 's/.*printf %s \([A-Za-z0-9+\/=]*\) .*/\1/' | base64 -d > "$d/session-$n.script" 2>/dev/null ;;
esac
cat "$d/banner" 2>/dev/null
exec sleep 60
"#;

/// The stand-in `ssh-keygen`: keeps its arguments in `keygen-args`, and prints the fingerprint
/// in the `fp` file when there is one.
const FAKE_KEYGEN: &str = "#!/bin/sh\n\
d=@DIR@\n\
printf '%s\\n' \"$@\" > \"$d/keygen-args\"\n\
cat \"$d/fp\" 2>/dev/null\n\
exit 0\n";

/// Copies the two paths after `--`, dropping the `fake:` host prefix.
const FAKE_SCP: &str = "#!/bin/sh\n\
seen=no; src=; dst=\n\
for a; do\n\
  if [ \"$a\" = -- ]; then seen=yes; continue; fi\n\
  [ $seen = yes ] || continue\n\
  case $a in fake:*) a=${a#fake:} ;; esac\n\
  if [ -z \"$src\" ]; then src=$a; else dst=$a; fi\n\
done\n\
exec cp -- \"$src\" \"$dst\"\n";

#[derive(Clone, Default)]
pub(super) struct RecordLinks(pub Rc<RefCell<Vec<String>>>);

impl Opener for RecordLinks {
    fn open(&self, target: &LinkTarget) -> Result<(), OpenError> {
        self.0.borrow_mut().push(target.as_str().to_owned());
        Ok(())
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordFiles(pub Rc<RefCell<Vec<PathBuf>>>);

impl FileShell for RecordFiles {
    fn open(&self, path: &Path) -> Result<(), OpenError> {
        self.0.borrow_mut().push(path.to_path_buf());
        Ok(())
    }

    fn reveal(&self, path: &Path) -> Result<(), OpenError> {
        self.open(path.parent().unwrap_or(path))
    }
}

/// One hub with stand-in programs.
pub(super) struct Rig {
    pub hub: Hub,
    pub panes: UnboundedReceiver<(PaneId, PaneEvent)>,
    pub jobs: UnboundedReceiver<SshEvent>,
    /// What the helper's jobs and the connecting `ssh` report.
    pub remote: UnboundedReceiver<RemoteEvent>,
    /// The stand-ins' folder: `up`, `banner`, `count`, `session-N`.
    pub dir: PathBuf,
    pub links: RecordLinks,
    pub files: RecordFiles,
    root: PathBuf,
}

impl Rig {
    /// A rig in scratch folders named `name`; `None` when `setsid` is missing (the tests that
    /// need it skip, unless `TEMOR_REQUIRE_TOOLS=1`).
    pub fn new(name: &str) -> Option<Rig> {
        let path = std::ffi::OsStr::new("/usr/bin:/bin");
        let Some(setsid) = temor_pty::find_program("setsid", path) else {
            if std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1") {
                panic!("setsid is required (TEMOR_REQUIRE_TOOLS=1)");
            }
            eprintln!("SKIPPED: needs util-linux setsid");
            return None;
        };
        // Short, because a shared connection's socket path has to fit a unix socket.
        let root = std::env::temp_dir().join(format!("tmr-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (home, bin, dir) = (root.join("home"), root.join("bin"), root.join("d"));
        for folder in [&home, &bin, &dir, &root.join("xdg")] {
            std::fs::create_dir_all(folder).expect("scratch folder");
        }
        write_tool(
            &bin.join("ssh"),
            &FAKE_SSH.replace("@DIR@", &dir.display().to_string()),
        );
        write_tool(&bin.join("scp"), FAKE_SCP);
        write_tool(
            &bin.join("ssh-keygen"),
            &FAKE_KEYGEN.replace("@DIR@", &dir.display().to_string()),
        );
        let search = format!("{}:/usr/bin:/bin", bin.display());
        let ambient = Ambient::from_vars(
            [
                ("HOME", home.as_os_str()),
                ("PATH", std::ffi::OsStr::new(&search)),
                ("XDG_RUNTIME_DIR", root.join("xdg").as_os_str()),
            ]
            .map(|(k, v)| (k.into(), v.to_os_string())),
        )
        .expect("an ambient with a home");
        let launch = Launch {
            appearance: AppearanceSource::scratch(
                home.join("config"),
                ds::prelude::SystemPrefs::default(),
            ),
            keymap: crate::edge::keymap_for(
                chordkit::Platform::Linux {
                    desktop: chordkit::Desktop::Ours,
                },
                &chordkit::Overrides::default(),
            )
            .0,
            launcher: LauncherChoice::Setsid(setsid),
            settings: temor_pane::settings::TerminalSettings::default(),
            ambient,
            probe: None,
            state_dir: None,
            first_words: Default::default(),
        };
        let mut hub = Hub::new(launch);
        let (links, files) = (RecordLinks::default(), RecordFiles::default());
        hub.set_openers(Box::new(links.clone()), Box::new(files.clone()));
        let panes = hub.inbox.take().expect("the pane events");
        let jobs = hub.ssh.inbox.take().expect("the job events");
        let remote = hub.remote.inbox.take().expect("the helper events");
        Some(Rig {
            hub,
            panes,
            jobs,
            remote,
            dir,
            links,
            files,
            root,
        })
    }

    /// Hears the panes and the jobs, and ticks, until `done` or 20 seconds have passed.
    pub fn pump(&mut self, what: &str, done: impl Fn(&Hub) -> bool) {
        let start = Instant::now();
        while !done(&self.hub) {
            let mut batch = Vec::new();
            while let Ok(event) = self.panes.try_recv() {
                batch.push(event);
            }
            if !batch.is_empty() {
                let _ = self.hub.deliver(batch);
            }
            while let Ok(event) = self.jobs.try_recv() {
                // What the window would show the person stays on the hub for the test to read.
                let said = self.hub.ssh_event(event);
                self.hub.notices.extend(said);
            }
            while let Ok(event) = self.remote.try_recv() {
                let said = self.hub.remote_event(event);
                self.hub.notices.extend(said);
            }
            let (_, said) = self.hub.ssh_tick();
            self.hub.notices.extend(said);
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The pane with the keys.
    pub fn front(&self) -> PaneId {
        self.hub.keyboard_pane().expect("a pane has the keys")
    }

    pub fn view(&self, pane: PaneId) -> HostView {
        self.hub.host_view(pane).expect("a host pane")
    }

    pub fn stage(&self, pane: PaneId) -> Stage {
        self.view(pane).stage
    }

    /// Writes a file into the stand-ins' folder.
    pub fn put(&self, name: &str, text: &str) {
        std::fs::write(self.dir.join(name), text).expect("stand-in file");
    }

    /// The file `name` of the stand-ins' folder, once it exists.
    pub fn got(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(name)).ok()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Writes an executable stand-in.
fn write_tool(path: &Path, body: &str) {
    std::fs::write(path, body).expect("stand-in tool");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

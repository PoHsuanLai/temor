//! Shared helpers: a daemon in this process, clients connected over pipes, a bash session, and
//! reading what comes back.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use temor_pty::SessionLauncher;
use temor_remote::{
    Client, ClientEvent, DAEMON_CAPS, DEFAULT_IDLE, Daemon, DaemonConfig, Error, ManualClock, hello,
};
use temor_vt::{GridSize, Terminal, TerminalConfig, Vt};
use temor_wire::digest::ContentHash;
use temor_wire::hello::HelperVersion;
use temor_wire::ids::SessionId;
use temor_wire::message::{EnvVar, OpenSession, RemotePath};
use temor_wire::units::{CellPx, Cols, Rows, Size};

/// How long a test waits for something the host does.
const PATIENCE: Duration = Duration::from_secs(20);

/// The helper identity the test daemon announces.
pub const HELPER: HelperVersion = HelperVersion(ContentHash([9; 32]));

/// A daemon with scratch folders. Dropping it hangs up its sessions.
pub struct Rig {
    pub daemon: Arc<Daemon>,
    pub clock: Arc<ManualClock>,
    pub home: PathBuf,
    pub files: PathBuf,
}

impl Rig {
    pub fn new(name: &str) -> Self {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("remote-{name}"));
        let _ = std::fs::remove_dir_all(&base);
        let (home, files) = (base.join("home"), base.join("files"));
        for dir in [&home, &files] {
            std::fs::create_dir_all(dir).expect("scratch folder");
        }
        let clock = Arc::new(ManualClock::new());
        let config = DaemonConfig {
            helper: HELPER,
            // The helper binary is its own session leader trampoline, as on a real host.
            launcher: SessionLauncher::Trampoline {
                program: env!("CARGO_BIN_EXE_temor-remote").into(),
                args: vec!["leader".into()],
            },
            default_shell: bash_path(),
            runtime_dir: base.join("run"),
            file_roots: vec![files.clone()],
            proc_root: PathBuf::from("/proc"),
            idle: DEFAULT_IDLE,
            clock: Arc::clone(&clock) as Arc<dyn temor_remote::Clock>,
        };
        Self {
            daemon: Daemon::new(config),
            clock,
            home,
            files,
        }
    }

    /// A new connection to the daemon, as a reconnecting window would make.
    pub fn connect(&self) -> Conn {
        self.connect_with(hello(HELPER, &DAEMON_CAPS))
    }

    pub fn connect_with(&self, ours: temor_wire::hello::Hello) -> Conn {
        let (served_in, client_out) = std::io::pipe().expect("pipe");
        let (client_in, served_out) = std::io::pipe().expect("pipe");
        let daemon = Arc::clone(&self.daemon);
        let serving = std::thread::spawn(move || daemon.serve(served_in, served_out));
        let result = Client::connect(client_in, client_out, ours);
        match result {
            Ok((client, events)) => Conn {
                client,
                watch: Watch::new(events),
                serving,
            },
            Err(error) => panic!("connect failed: {error}"),
        }
    }

    /// A bash in the scratch HOME with a fixed prompt, 80 by 24, and temor's shell integration
    /// off so the screen holds only what a test makes it hold.
    pub fn bash(&self) -> OpenSession {
        let mut open = self.bash_with_marks();
        open.env.push(EnvVar {
            name: "TEMOR_SHELL_INTEGRATION".to_string(),
            value: "0".to_string(),
        });
        open
    }

    /// The same bash with the integration on, as the window opens it.
    pub fn bash_with_marks(&self) -> OpenSession {
        let env = [
            ("HOME", self.home.display().to_string()),
            ("PATH", "/usr/bin:/bin".to_string()),
            ("TERM", "xterm-256color".to_string()),
            ("LANG", "C.UTF-8".to_string()),
            ("PS1", "$ ".to_string()),
            ("HISTFILE", self.home.join("history").display().to_string()),
        ];
        OpenSession {
            shell: Some(RemotePath(bash_path().display().to_string())),
            cwd: RemotePath(self.home.display().to_string()),
            env: env
                .into_iter()
                .map(|(name, value)| EnvVar {
                    name: name.to_string(),
                    value,
                })
                .collect(),
            size: Size {
                cols: Cols(80),
                rows: Rows(24),
            },
            cell: CellPx::DEFAULT,
            palette: None,
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.daemon.shutdown();
    }
}

/// A connected client, and the thread serving the other end.
pub struct Conn {
    pub client: Client,
    pub watch: Watch,
    serving: JoinHandle<Result<(), Error>>,
}

impl Conn {
    /// Drops the connection, as a lost network would, and returns how the daemon saw it end.
    pub fn hang_up(self) -> Result<(), Error> {
        drop(self.client);
        drop(self.watch);
        self.serving
            .join()
            .expect("the serving thread did not panic")
    }
}

/// What came over a link: output collected per session, and every other event kept in order.
pub struct Watch {
    events: Receiver<ClientEvent>,
    output: Vec<u8>,
    pub others: Vec<ClientEvent>,
}

impl Watch {
    fn new(events: Receiver<ClientEvent>) -> Self {
        Self {
            events,
            output: Vec::new(),
            others: Vec::new(),
        }
    }

    /// Waits until `ready` holds for the output so far; the output, or a panic naming `what`.
    pub fn output_has(&mut self, what: &str, ready: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let text = String::from_utf8_lossy(&self.output).into_owned();
            if ready(&text) {
                return text;
            }
            assert!(
                self.pump(deadline),
                "timed out waiting for {what}; output so far:\n{text}"
            );
        }
    }

    /// Waits for an event other than output that `pick` accepts.
    pub fn event<T>(&mut self, what: &str, pick: impl Fn(&ClientEvent) -> Option<T>) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(found) = self.others.iter().find_map(&pick) {
                return found;
            }
            assert!(
                self.pump(deadline),
                "timed out waiting for {what}; events so far: {:?}",
                self.others
            );
        }
    }

    /// Reads one event into the buffers; false once the deadline has passed or the link is gone.
    fn pump(&mut self, deadline: Instant) -> bool {
        let wait = deadline.saturating_duration_since(Instant::now());
        match self
            .events
            .recv_timeout(wait.min(Duration::from_millis(200)))
        {
            Ok(ClientEvent::Output { bytes, .. }) => {
                self.output.extend_from_slice(&bytes);
                true
            }
            Ok(other) => {
                self.others.push(other);
                true
            }
            Err(RecvTimeoutError::Timeout) => Instant::now() < deadline,
            Err(RecvTimeoutError::Disconnected) => false,
        }
    }
}

/// Polls `check` until it returns `Some`, or panics naming `what`.
pub fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// The screen a snapshot holds, as text: scrollback and screen, trailing blanks trimmed.
pub fn snapshot_text(attached: &temor_remote::Attached) -> String {
    let (events, _events_rx) = std::sync::mpsc::channel::<temor_vt::VtEvent>();
    let (replies, _replies_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let screen = Terminal::restore(
        &attached.screen(),
        TerminalConfig::new(GridSize::new(80, 24)),
        Box::new(events),
        Box::new(replies),
    )
    .expect("the snapshot restores");
    screen.plain_text().expect("the screen reads back")
}

/// The id of the one session a listing holds.
pub fn only_session(conn: &Conn) -> Option<SessionId> {
    let sessions = conn.client.list_sessions().ok()?;
    match sessions.as_slice() {
        [one] => Some(one.session),
        _ => None,
    }
}

pub fn bash_path() -> PathBuf {
    temor_pty::find_program("bash", std::ffi::OsStr::new("/usr/bin:/bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/bash"))
}

/// Whether bash and `/proc` are there. Where not, the terminal tests return early with a note,
/// or fail under TEMOR_REQUIRE_TOOLS=1 so a gate that has the tools cannot pass silently.
pub fn tools_present() -> bool {
    let found = bash_path().exists() && Path::new("/proc/self").exists();
    if !found {
        assert!(
            !std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1"),
            "bash and /proc are required (TEMOR_REQUIRE_TOOLS=1)"
        );
        eprintln!("SKIPPED: needs bash and /proc (set TEMOR_REQUIRE_TOOLS=1 to fail instead)");
    }
    found
}

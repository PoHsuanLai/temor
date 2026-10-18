//! Single instance over the session bus: the first launch owns `org.quire.Temor1`, and a later
//! launch calls `NewWindow`, `NewTab` or `Run` on it. The `.service` files in `dist/` let the bus
//! start temor when someone calls either name while none runs.
//!
//! `Run` starts any program, outside any sandbox the caller is in, so it answers only temor's own
//! forwarding launch (`temor -e`): peephole holds the caller's process from the moment it
//! connected and judges it against [`CALLERS`], the images temor is installed as. A sandboxed app,
//! another program, and a temor the person built themselves (an image they can write) are all
//! refused. `NewWindow` and `NewTab` open only a shell, and stay open to anyone.

use super::bus::session_builder;
use crate::{
    CLOSED_WHILE_RUNNING, Claim, DesktopEnv, DesktopError, Handover, INSTANCE_BUS_NAME, Primary,
    Request, RunEnd, RunEnds, RunRequest, RunTicket,
};
use futures_util::StreamExt;
use peephole_bus::peephole_core::{PinNeed, Proof, Requirement, RoleName, Table};
use peephole_bus::{BusPeers, ManagerCheck, Offload};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::names::OwnedUniqueName;
use zbus::{MatchRule, MessageStream};
use zbus::{fdo, interface};

const OBJECT_PATH: &str = "/org/quire/Temor1";

/// The signal a waiting launch hears when its tab closed: `(ticket t, status i)`.
const RUN_ENDED: &str = "RunEnded";

/// The launches waiting on their tab, by ticket: where to send `RunEnded`.
type Waiting = Arc<Mutex<HashMap<RunTicket, OwnedUniqueName>>>;

/// The programs that may ask a running temor to run something: temor as the desktop installs it
/// (root-owned all the way up). A row with no unit is known by its image alone, so `Exe` is the
/// most it can show and all `Run` asks for.
const CALLERS: &str = r#"
[[peer]]
app  = "org.quire.Temor"
exe  = [
  "/opt/quire/releases/*/bin/temor",
  "/opt/quire/current/bin/temor",
  "/usr/local/bin/temor",
  "/usr/bin/temor",
]
role = "terminal"
"#;

/// The role [`CALLERS`] gives temor's own images.
const TERMINAL: RoleName = RoleName::from_static("terminal");

/// Who may call `Run`: temor's own image, held from connect time.
fn run_callers() -> [Requirement; 1] {
    [Requirement::new(TERMINAL, Proof::Exe).pinned(PinNeed::AtConnect)]
}

/// The callers of the primary's bus connection, once it has one.
type Peers = Arc<OnceLock<BusPeers<peephole_bus::peephole::RealProc>>>;

/// Single instance on the session bus.
#[derive(Debug, Clone)]
pub struct SystemInstance {
    env: DesktopEnv,
    peers: Peers,
}

impl SystemInstance {
    /// An instance claim in `env`.
    pub fn new(env: DesktopEnv) -> Self {
        Self {
            env,
            peers: Peers::default(),
        }
    }

    /// Makes the connection named `unique` stand as temor's own image for `Run`, without looking
    /// at its process: a test's own connection, since a test binary is no installed temor. Only
    /// after this instance became the primary.
    #[cfg(feature = "callers-for-tests")]
    pub fn vouch_for(&self, unique: &str) {
        use peephole_bus::peephole_core::{AppKey, Pin, Standing};
        let peers = self.peers.get().expect("the primary's callers");
        peers.introduce(
            unique,
            Standing::Service {
                app: AppKey::new("org.quire.Temor").expect("an app key"),
                role: TERMINAL,
                proof: Proof::Exe,
                pin: Pin::AtConnect,
            },
        );
    }

    /// What a later launch does, on a connection already open: a test's own, which it named
    /// with [`SystemInstance::vouch_for`] first.
    #[cfg(feature = "callers-for-tests")]
    pub async fn forward_for_tests(
        connection: &zbus::Connection,
        request: &Request,
        token: Option<&str>,
    ) -> Result<(), DesktopError> {
        forward_on(connection, request, token).await
    }

    /// [`SystemInstance::run_and_wait`] on a connection already open: a test's own, vouched for.
    #[cfg(feature = "callers-for-tests")]
    pub async fn run_and_wait_for_tests(
        connection: &zbus::Connection,
        run: &RunRequest,
        token: Option<&str>,
    ) -> Result<i32, DesktopError> {
        wait_on(connection, run, token).await
    }

    /// `temor -e`: hands `run` to the running temor (the bus starts one when it can) and waits
    /// until its tab closes, for the status to exit with: the program's, or
    /// [`CLOSED_WHILE_RUNNING`] when the tab, or temor, went while it ran. `None` when no temor
    /// runs and the bus cannot start one: this launch is then the window itself.
    pub async fn run_and_wait(
        &self,
        run: &RunRequest,
        token: Option<&str>,
    ) -> Result<Option<i32>, DesktopError> {
        let connection = super::bus::connect(&self.env).await?;
        let bus = DBusProxy::new(&connection)
            .await
            .map_err(|error| DesktopError::bus("ask the bus about temor", error))?;
        let name = zbus::names::BusName::try_from(INSTANCE_BUS_NAME)
            .map_err(|error| DesktopError::bus("name temor", error))?;
        let running = bus.name_has_owner(name.clone()).await.unwrap_or(false);
        let startable = bus.list_activatable_names().await.is_ok_and(|names| {
            names
                .iter()
                .any(|known| known.as_str() == INSTANCE_BUS_NAME)
        });
        if !running && !startable {
            return Ok(None);
        }
        wait_on(&connection, run, token).await.map(Some)
    }

    /// Try to be the first temor. The first launch owns the name and gets [`Claim::Primary`];
    /// a later one asks the first to do `request` and gets [`Claim::Forwarded`].
    pub async fn claim(&self, request: &Request) -> Result<Claim, DesktopError> {
        self.claim_with(request, None).await
    }

    /// [`claim`](Self::claim), handing the first temor `token` (this launch's activation token)
    /// along with the request so it can raise its window.
    pub async fn claim_with(
        &self,
        request: &Request,
        token: Option<&str>,
    ) -> Result<Claim, DesktopError> {
        let (sender, requests) = unbounded_channel();
        let waiting = Waiting::default();
        let service = Service {
            sender,
            peers: Arc::clone(&self.peers),
            waiting: Arc::clone(&waiting),
            next: AtomicU64::new(1),
        };
        let built = session_builder(&self.env)?
            .name(INSTANCE_BUS_NAME)
            // The first temor keeps the name: a later one must be told it is taken, not take it.
            .map(|builder| {
                builder
                    .allow_name_replacements(false)
                    .replace_existing_names(false)
            })
            .and_then(|builder| builder.serve_at(OBJECT_PATH, service))
            .map_err(|error| DesktopError::bus("register temor's name", error))?
            .build()
            .await;
        match built {
            Ok(connection) => {
                let table = Table::from_toml(CALLERS)
                    .map_err(|error| DesktopError::bus("read temor's caller table", error))?;
                // `Run` never asks for the unit manager's word (`Full`), so it is never trusted.
                let manager = ManagerCheck::Verified(Requirement::new(
                    RoleName::from_static("manager"),
                    Proof::Full,
                ));
                // A connection of its own: the served object holds the callers, so the primary's
                // connection there would keep itself, and the name, alive.
                let asking = super::bus::connect(&self.env).await?;
                let peers = BusPeers::new(asking, table, manager).with_offload(Offload::Tokio);
                let _ = self.peers.set(peers);
                let (ends, heard) = unbounded_channel();
                tokio::spawn(tell_waiting(connection.clone(), waiting, heard));
                let primary = Primary::new(requests, Box::new(connection));
                Ok(Claim::Primary(primary.with_run_ends(RunEnds::new(ends))))
            }
            Err(zbus::Error::NameTaken) => {
                forward(&self.env, request, token).await?;
                Ok(Claim::Forwarded)
            }
            Err(error) => Err(DesktopError::bus("register temor's name", error)),
        }
    }
}

/// Call the running temor with `request`. The bus starts temor for the call when none runs and
/// the activation file is installed.
async fn forward(
    env: &DesktopEnv,
    request: &Request,
    token: Option<&str>,
) -> Result<(), DesktopError> {
    let connection = super::bus::connect(env).await?;
    forward_on(&connection, request, token).await
}

async fn forward_on(
    connection: &zbus::Connection,
    request: &Request,
    token: Option<&str>,
) -> Result<(), DesktopError> {
    let token = token.unwrap_or_default().to_owned();
    let reply = match request {
        Request::NewWindow => {
            connection
                .call_method(
                    Some(INSTANCE_BUS_NAME),
                    OBJECT_PATH,
                    Some(INSTANCE_BUS_NAME),
                    "NewWindow",
                    &(token,),
                )
                .await
        }
        Request::NewTab { cwd } => {
            let cwd = cwd
                .as_deref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            connection
                .call_method(
                    Some(INSTANCE_BUS_NAME),
                    OBJECT_PATH,
                    Some(INSTANCE_BUS_NAME),
                    "NewTab",
                    &(cwd, token),
                )
                .await
        }
        Request::Run(run) => call_run(connection, run, Some(&token)).await,
    };
    reply
        .map(drop)
        .map_err(|error| DesktopError::bus("ask the running temor to open it", error))
}

/// Sends `RunEnded` to each launch whose tab closed, until the window can say no more (every
/// [`RunEnds`] is gone, so this holds the connection no longer than the primary does).
async fn tell_waiting(
    connection: zbus::Connection,
    waiting: Waiting,
    mut heard: tokio::sync::mpsc::UnboundedReceiver<RunEnd>,
) {
    while let Some(end) = heard.recv().await {
        let launch = waiting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&end.ticket);
        if let Some(launch) = launch {
            // Only the launch that waits hears it.
            let _ = connection
                .emit_signal(
                    Some(&launch),
                    OBJECT_PATH,
                    INSTANCE_BUS_NAME,
                    RUN_ENDED,
                    &(end.ticket.0, end.status),
                )
                .await;
        }
    }
}

/// Asks the running temor to run `run` on `connection` and waits for its tab to close.
async fn wait_on(
    connection: &zbus::Connection,
    run: &RunRequest,
    token: Option<&str>,
) -> Result<i32, DesktopError> {
    let failed = |error: zbus::Error| DesktopError::bus("wait for temor's tab", error);
    // Both are heard from before the call, so nothing said after it is missed.
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(INSTANCE_BUS_NAME)
        .map_err(failed)?
        .member(RUN_ENDED)
        .map_err(failed)?
        .path(OBJECT_PATH)
        .map_err(failed)?
        .build();
    let mut ended = MessageStream::for_match_rule(rule, connection, None)
        .await
        .map_err(failed)?;
    let bus = DBusProxy::new(connection).await.map_err(failed)?;
    let mut owners = bus
        .receive_name_owner_changed_with_args(&[(0, INSTANCE_BUS_NAME)])
        .await
        .map_err(failed)?;
    let reply = call_run(connection, run, token)
        .await
        .map_err(|error| DesktopError::bus("ask the running temor to run it", error))?;
    let ticket: u64 = reply.body().deserialize().map_err(failed)?;
    // The temor that answered, as the reply says: asking the bus afterwards could find it gone.
    let Some(owner) = reply.header().sender().map(|name| name.to_owned()) else {
        return Ok(CLOSED_WHILE_RUNNING);
    };
    loop {
        tokio::select! {
            message = ended.next() => {
                let Some(Ok(message)) = message else {
                    return Ok(CLOSED_WHILE_RUNNING);
                };
                let from_owner = message.header().sender().is_some_and(|s| *s == *owner);
                if let (true, Ok((heard, status))) =
                    (from_owner, message.body().deserialize::<(u64, i32)>())
                    && heard == ticket
                {
                    return Ok(status);
                }
            }
            changed = owners.next() => {
                // temor went away: its tabs closed with it.
                let Some(changed) = changed else {
                    return Ok(CLOSED_WHILE_RUNNING);
                };
                if let Ok(args) = changed.args()
                    && args.old_owner().as_ref().is_some_and(|old| *old == *owner)
                {
                    return Ok(CLOSED_WHILE_RUNNING);
                }
            }
        }
    }
}

async fn call_run(
    connection: &zbus::Connection,
    run: &RunRequest,
    token: Option<&str>,
) -> zbus::Result<zbus::Message> {
    let argv: Vec<&[u8]> = run.argv.iter().map(|arg| arg.as_bytes()).collect();
    let cwd = run.cwd.as_deref().map(|path| path.as_os_str().as_bytes());
    let env: Vec<(&[u8], &[u8])> = run
        .env
        .iter()
        .map(|(key, value)| (key.as_bytes(), value.as_bytes()))
        .collect();
    connection
        .call_method(
            Some(INSTANCE_BUS_NAME),
            OBJECT_PATH,
            Some(INSTANCE_BUS_NAME),
            "Run",
            &(
                argv,
                cwd.unwrap_or_default(),
                env,
                token.unwrap_or_default(),
            ),
        )
        .await
}

/// The object other launches call.
struct Service {
    sender: UnboundedSender<Handover>,
    peers: Peers,
    waiting: Waiting,
    next: AtomicU64,
}

impl Service {
    fn hand_over(&self, request: Request, token: String) -> fdo::Result<()> {
        let token = (!token.is_empty()).then_some(token);
        self.sender
            .send(Handover { request, token })
            .map_err(|_| fdo::Error::Failed("temor is closing".to_owned()))
    }
}

#[interface(name = "org.quire.Temor1")]
impl Service {
    /// Open a new window; `token` is the caller's activation token, empty when it has none.
    fn new_window(&self, token: String) -> fdo::Result<()> {
        self.hand_over(Request::NewWindow, token)
    }

    /// Open a new tab in `cwd`; an empty folder means the default one.
    fn new_tab(&self, cwd: String, token: String) -> fdo::Result<()> {
        let cwd = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
        self.hand_over(Request::NewTab { cwd }, token)
    }

    /// Run `argv` in a new tab in place of the shell, in `cwd` (empty: the default folder) with
    /// `env`. Each is the caller's bytes, as the system keeps them. Only temor's own launch may
    /// ask; anyone else is refused with `AccessDenied`. Returns the ticket the caller waits on:
    /// `RunEnded(ticket, status)` comes to it alone once the tab closes.
    async fn run(
        &self,
        argv: Vec<Vec<u8>>,
        cwd: Vec<u8>,
        env: Vec<(Vec<u8>, Vec<u8>)>,
        token: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<u64> {
        let Some(peers) = self.peers.get() else {
            return Err(fdo::Error::AccessDenied(
                "temor is still starting".to_owned(),
            ));
        };
        peers
            .require_or_deny(header.sender().map(|name| name.as_str()), &run_callers())
            .await?;
        if argv.first().is_none_or(Vec::is_empty) {
            return Err(fdo::Error::InvalidArgs("no program to run".to_owned()));
        }
        let Some(caller) = header.sender().map(|name| name.to_owned()) else {
            return Err(fdo::Error::AccessDenied("no caller".to_owned()));
        };
        let ticket = RunTicket(self.next.fetch_add(1, Ordering::Relaxed));
        let run = RunRequest {
            argv: argv.into_iter().map(OsString::from_vec).collect(),
            cwd: (!cwd.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(&cwd))),
            env: env
                .into_iter()
                .map(|(key, value)| (OsString::from_vec(key), OsString::from_vec(value)))
                .collect(),
            ticket: Some(ticket),
        };
        self.waiting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(ticket, caller.into());
        self.hand_over(Request::Run(run), token).inspect_err(|_| {
            self.waiting
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&ticket);
        })?;
        Ok(ticket.0)
    }
}

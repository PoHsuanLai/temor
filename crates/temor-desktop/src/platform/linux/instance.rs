//! Single instance over the session bus: the first launch owns `org.quire.Temor1`, and a later
//! launch calls `NewWindow` or `NewTab` on it. The `.service` files in `dist/` let the bus start
//! temor when someone calls either name while none runs.

use super::bus::session_builder;
use crate::{Claim, DesktopEnv, DesktopError, INSTANCE_BUS_NAME, Primary, Request};
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use zbus::{fdo, interface};

const OBJECT_PATH: &str = "/org/quire/Temor1";

/// Single instance on the session bus.
#[derive(Debug, Clone)]
pub struct SystemInstance {
    env: DesktopEnv,
}

impl SystemInstance {
    /// An instance claim in `env`.
    pub fn new(env: DesktopEnv) -> Self {
        Self { env }
    }

    /// Try to be the first temor. The first launch owns the name and gets [`Claim::Primary`];
    /// a later one asks the first to do `request` and gets [`Claim::Forwarded`].
    pub async fn claim(&self, request: &Request) -> Result<Claim, DesktopError> {
        let (sender, requests) = unbounded_channel();
        let built = session_builder(&self.env)?
            .name(INSTANCE_BUS_NAME)
            // The first temor keeps the name: a later one must be told it is taken, not take it.
            .map(|builder| {
                builder
                    .allow_name_replacements(false)
                    .replace_existing_names(false)
            })
            .and_then(|builder| builder.serve_at(OBJECT_PATH, Service { sender }))
            .map_err(|error| DesktopError::bus("register temor's name", error))?
            .build()
            .await;
        match built {
            Ok(connection) => Ok(Claim::Primary(Primary::new(requests, Box::new(connection)))),
            Err(zbus::Error::NameTaken) => {
                forward(&self.env, request).await?;
                Ok(Claim::Forwarded)
            }
            Err(error) => Err(DesktopError::bus("register temor's name", error)),
        }
    }
}

/// Call the running temor with `request`. The bus starts temor for the call when none runs and
/// the activation file is installed.
async fn forward(env: &DesktopEnv, request: &Request) -> Result<(), DesktopError> {
    let connection = super::bus::connect(env).await?;
    let reply = match request {
        Request::NewWindow => {
            connection
                .call_method(
                    Some(INSTANCE_BUS_NAME),
                    OBJECT_PATH,
                    Some(INSTANCE_BUS_NAME),
                    "NewWindow",
                    &(),
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
                    &(cwd,),
                )
                .await
        }
    };
    reply
        .map(drop)
        .map_err(|error| DesktopError::bus("ask the running temor to open it", error))
}

/// The object other launches call.
struct Service {
    sender: UnboundedSender<Request>,
}

impl Service {
    fn hand_over(&self, request: Request) -> fdo::Result<()> {
        self.sender
            .send(request)
            .map_err(|_| fdo::Error::Failed("temor is closing".to_owned()))
    }
}

#[interface(name = "org.quire.Temor1")]
impl Service {
    /// Open a new window.
    fn new_window(&self) -> fdo::Result<()> {
        self.hand_over(Request::NewWindow)
    }

    /// Open a new tab in `cwd`; an empty folder means the default one.
    fn new_tab(&self, cwd: String) -> fdo::Result<()> {
        let cwd = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
        self.hand_over(Request::NewTab { cwd })
    }
}

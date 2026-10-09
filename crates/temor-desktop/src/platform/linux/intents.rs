//! temor as an app of docket, the desktop's agent layer: `org.quire.IntentProvider1` served on
//! the app's own bus name, `org.quire.Temor`, so the router can ask the window to list panes,
//! read command output, open panes and run commands. The declarations are the embedded
//! `dist/intents/org.quire.Temor.toml`.

use super::bus::connect;
use super::context::{WindowSource, app_name};
use super::perform::{RUN_TOKEN, class_of, pane_of_token, perform};
use crate::{COMMAND_RUN_SANDBOXED, DesktopEnv, DesktopError, MANIFEST, WindowPort};
use docket_client::{IntentProvider, SummonTarget};
use docket_core::{
    AppRefusal, CallClass, EntityRef, Hit, Invocation, Manifest, Outcome, Preview, SuggestAsk,
    SummonAnswer, SummonOrigin, SummonSerial, UndoFault, UndoToken, ValidManifest, validate,
};
use porter_core::AppName;
use prov::{Actor, EntityId};
use std::sync::Arc;

/// temor's actions, served to the agent layer until this value is dropped.
#[derive(Debug)]
pub struct SystemIntents {
    _connection: zbus::Connection,
}

impl SystemIntents {
    /// Serve temor's actions on the session bus of `env`, answered by `window`. Claims the bus
    /// name `org.quire.Temor`; the router calls it only after checking the owner is temor.
    pub async fn serve<W: WindowPort>(
        env: &DesktopEnv,
        window: Arc<W>,
    ) -> Result<Self, DesktopError> {
        let manifest = load_manifest()?;
        let app = app_name().ok_or_else(|| DesktopError::Manifest("bad app name".to_owned()))?;
        let connection = connect(env).await?;
        let source = WindowSource {
            app: app.clone(),
            window: window.clone(),
        };
        let provider = TemorIntents {
            manifest,
            app,
            window,
        };
        docket_client::serve_on(&connection, provider, source, NoPrompt)
            .await
            .map_err(|error| DesktopError::bus("offer temor to the agent layer", error))?;
        Ok(Self {
            _connection: connection,
        })
    }
}

/// The shipped file, validated the way the router validates it.
pub(super) fn load_manifest() -> Result<ValidManifest, DesktopError> {
    let parsed: Manifest =
        toml::from_str(MANIFEST).map_err(|error| DesktopError::Manifest(error.to_string()))?;
    validate(parsed).map_err(|error| DesktopError::Manifest(error.to_string()))
}

/// Answers the router's calls from the window.
struct TemorIntents<W> {
    manifest: ValidManifest,
    app: AppName,
    window: Arc<W>,
}

impl<W: WindowPort> IntentProvider for TemorIntents<W> {
    fn manifest(&self) -> &ValidManifest {
        &self.manifest
    }

    async fn perform(&self, inv: Invocation) -> Result<Outcome, AppRefusal> {
        perform(self.window.as_ref(), &self.app, inv).await
    }

    async fn classify(&self, inv: Invocation) -> Result<CallClass, AppRefusal> {
        if inv.action.as_str() != COMMAND_RUN_SANDBOXED {
            return Err(AppRefusal::Unsupported);
        }
        let network = super::args::Reader::new(&inv.args).network()?;
        Ok(CallClass::Effect(class_of(network)))
    }

    async fn dry_run(&self, _inv: Invocation) -> Result<Preview, AppRefusal> {
        Err(AppRefusal::Unsupported)
    }

    async fn undo(&self, token: UndoToken, _actor: Actor) -> Result<(), UndoFault> {
        let token = token.as_str();
        let done = match (pane_of_token(token), token.strip_prefix(RUN_TOKEN)) {
            (Some(pane), _) => self.window.close_pane(pane),
            (None, Some(run)) => self.window.undo(run),
            (None, None) => return Err(UndoFault::Gone),
        };
        done.map_err(|why| match why {
            crate::PortRefusal::Busy => UndoFault::AppUnavailable,
            _ => UndoFault::Gone,
        })
    }

    async fn search(&self, _text: &str) -> Vec<Hit> {
        Vec::new()
    }

    async fn preview(&self, _id: &EntityId) -> Preview {
        Preview::None
    }

    async fn suggest(&self, _ask: SuggestAsk) -> Vec<EntityRef> {
        Vec::new()
    }
}

/// temor has no prompt field for the companion to summon into yet (P12).
struct NoPrompt;

impl SummonTarget for NoPrompt {
    fn summon(&self, _serial: SummonSerial, _origin: SummonOrigin) -> SummonAnswer {
        SummonAnswer::Declined
    }
}

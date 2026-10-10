//! temor offered to the agent layer: a call from "the router" (a connection that owns the name a
//! provider trusts, on a bus of its own) reaches the window, and nobody else is answered.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use crate::window::FakeWindow;
use docket_core::{
    AppRefusal, Args, CallClass, CallId, ContextSnapshot, Invocation, Origin, Outcome, ParamName,
    TargetValue, UndoFault, Undoable, Value,
};
use porter_core::AppName;
use prov::{ActionName, Actor, Effect, Integrity, Label, Labelled, SpaceId};
use std::collections::HashMap;
use std::sync::Arc;
use temor_desktop::{APP_BUS_NAME, Network, PaneRef, SystemIntents};
use zbus::zvariant::Value as BusValue;

const PROVIDER_PATH: &str = "/org/quire/IntentProvider1";
const PROVIDER_INTERFACE: &str = "org.quire.IntentProvider1";
const ROUTER_NAME: &str = "org.quire.Intents1";

fn invocation(action: &str, args: &[(&str, Value)]) -> String {
    let args: Args = args
        .iter()
        .map(|(name, value)| {
            (
                ParamName::parse(name).unwrap(),
                Labelled::new(value.clone(), Label::trusted_user()),
            )
        })
        .collect();
    let call = Invocation {
        call: CallId(1),
        action: ActionName::parse(action).unwrap(),
        target: TargetValue::Nothing,
        args,
        actor: Actor::User {
            via: AppName::parse("org.quire.Shell").unwrap(),
        },
        origin: Origin::Launcher,
        space: SpaceId::parse("work").unwrap(),
    };
    serde_json::to_string(&call).unwrap()
}

fn choice(id: &str) -> Value {
    Value::Choice(docket_core::ChoiceId::parse(id).unwrap())
}

/// The router's call of `member` on temor, with `arguments`, as `caller`.
async fn call(
    caller: &zbus::Connection,
    member: &str,
    arguments: &(impl serde::Serialize + zbus::zvariant::DynamicType),
) -> zbus::Result<String> {
    let reply = caller
        .call_method(
            Some(APP_BUS_NAME),
            PROVIDER_PATH,
            Some(PROVIDER_INTERFACE),
            member,
            arguments,
        )
        .await?;
    reply.body().deserialize()
}

async fn perform(
    router: &zbus::Connection,
    action: &str,
    args: &[(&str, Value)],
) -> Result<Outcome, AppRefusal> {
    let options: HashMap<String, BusValue<'_>> = HashMap::new();
    let text = call(router, "Perform", &(invocation(action, args), options))
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

struct Served {
    _bus: PrivateBus,
    window: Arc<FakeWindow>,
    router: zbus::Connection,
    _served: SystemIntents,
}

async fn served() -> Option<Served> {
    let bus = PrivateBus::start()?;
    let window = Arc::new(FakeWindow::default());
    let served = SystemIntents::serve(&bus.env(), window.clone())
        .await
        .unwrap();
    let router = bus.connect().await;
    router.request_name(ROUTER_NAME).await.unwrap();
    Some(Served {
        _bus: bus,
        window,
        router,
        _served: served,
    })
}

fn text_of(outcome: &Outcome) -> (&str, Integrity) {
    let value = outcome.value.as_ref().expect("a value");
    let Value::Text(text) = &value.value else {
        panic!("a text value");
    };
    (text, value.label.integrity)
}

#[tokio::test]
async fn reads_come_back_as_untrusted_text() {
    let Some(s) = served().await else { return };
    let listed = perform(&s.router, "temor.pane.list", &[]).await.unwrap();
    let (text, integrity) = text_of(&listed);
    assert!(text.contains("\"title\":\"zsh\""), "{text}");
    assert_eq!(integrity, Integrity::Untrusted);

    // The pane left out is the one in front; the range defaults and is passed on.
    let read = perform(&s.router, "temor.pane.read", &[]).await.unwrap();
    assert_eq!(text_of(&read).0, "lines 0+200");
    let cwd = perform(&s.router, "temor.pane.cwd", &[]).await.unwrap();
    assert_eq!(text_of(&cwd).0, "/home/me/src");

    let gone = perform(&s.router, "temor.pane.read", &[("pane", Value::Integer(5))]).await;
    assert!(matches!(gone, Err(AppRefusal::Failed(_))), "{gone:?}");
}

#[tokio::test]
async fn an_opened_pane_can_be_closed_again_with_its_undo_token() {
    let Some(s) = served().await else { return };
    let opened = perform(
        &s.router,
        "temor.pane.open",
        &[("placement", choice("split_right"))],
    )
    .await
    .unwrap();
    let Undoable::Yes(token) = opened.undo else {
        panic!("opening a pane is undoable");
    };
    assert_eq!(s.window.log.lock().unwrap().opened.len(), 1);

    let options = format!("\"{}\"", token.as_str());
    let actor = serde_json::to_string(&Actor::User {
        via: AppName::parse("org.quire.Shell").unwrap(),
    })
    .unwrap();
    let text = call(&s.router, "Undo", &(options, actor)).await.unwrap();
    let undone: Result<(), UndoFault> = serde_json::from_str(&text).unwrap();
    assert_eq!(undone, Ok(()));
    assert_eq!(s.window.log.lock().unwrap().closed, vec![PaneRef(9)]);
}

#[tokio::test]
async fn a_sandboxed_run_is_classified_by_its_network_and_undone_by_the_window() {
    let Some(s) = served().await else { return };
    for (network, want) in [
        ("closed", Effect::UndoableWrite),
        ("open", Effect::Outbound),
    ] {
        let args = [
            ("line", Value::Text("make".to_owned())),
            ("network", choice(network)),
        ];
        let options: HashMap<String, BusValue<'_>> = HashMap::new();
        let text = call(
            &s.router,
            "Classify",
            &(invocation("temor.command.run_sandboxed", &args), options),
        )
        .await
        .unwrap();
        let class: Result<CallClass, AppRefusal> = serde_json::from_str(&text).unwrap();
        assert_eq!(class, Ok(CallClass::Effect(want)), "{network}");
    }

    let ran = perform(
        &s.router,
        "temor.command.run_sandboxed",
        &[("line", Value::Text("make".to_owned()))],
    )
    .await
    .unwrap();
    assert!(text_of(&ran).0.ends_with("done"));
    assert_eq!(
        s.window.log.lock().unwrap().sandboxed[0].network,
        Network::Closed
    );
    let Undoable::Yes(token) = ran.undo else {
        panic!("a run that changed something is undoable");
    };
    let actor = serde_json::to_string(&Actor::User {
        via: AppName::parse("org.quire.Shell").unwrap(),
    })
    .unwrap();
    let quoted = format!("\"{}\"", token.as_str());
    call(&s.router, "Undo", &(quoted, actor)).await.unwrap();
    assert_eq!(
        s.window.log.lock().unwrap().undone,
        vec!["overlay-5".to_owned()]
    );
}

#[tokio::test]
async fn typing_into_a_pane_needs_the_line_and_reaches_the_front_pane() {
    let Some(s) = served().await else { return };
    let missing = perform(&s.router, "temor.command.run_in_pane", &[]).await;
    assert!(
        matches!(missing, Err(AppRefusal::NeedsParam { .. })),
        "{missing:?}"
    );
    assert!(s.window.log.lock().unwrap().typed.is_empty());

    perform(
        &s.router,
        "temor.command.run_in_pane",
        &[("line", Value::Text("ls".to_owned()))],
    )
    .await
    .unwrap();
    assert_eq!(
        s.window.log.lock().unwrap().typed,
        vec![(PaneRef(1), "ls".to_owned())]
    );
}

#[tokio::test]
async fn context_reports_the_window_and_only_the_router_may_ask() {
    let Some(s) = served().await else { return };
    let text = call(&s.router, "Context", &("\"active_window\"".to_owned(),))
        .await
        .unwrap();
    let snapshot: ContextSnapshot = serde_json::from_str(&text).unwrap();
    assert_eq!(snapshot.app.as_str(), APP_BUS_NAME);
    assert_eq!(snapshot.window.value, "temor");

    let intruder = s._bus.connect().await;
    let options: HashMap<String, BusValue<'_>> = HashMap::new();
    let refused = call(
        &intruder,
        "Perform",
        &(invocation("temor.pane.list", &[]), options),
    )
    .await;
    assert!(refused.is_err());
    assert!(s.window.log.lock().unwrap().typed.is_empty());
}

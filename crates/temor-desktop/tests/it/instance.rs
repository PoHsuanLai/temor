//! Single instance: the first launch owns the name, a second asks it to open a window or tab and
//! is told to exit.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;
use temor_desktop::{Claim, Primary, Request, RunRequest, RunTicket, SystemInstance};

async fn first(bus: &PrivateBus) -> Primary {
    match SystemInstance::new(bus.env())
        .claim(&Request::NewWindow)
        .await
        .unwrap()
    {
        Claim::Primary(primary) => primary,
        Claim::Forwarded => panic!("the first launch must own the name"),
    }
}

#[tokio::test]
async fn a_second_launch_hands_its_request_to_the_first_and_exits() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = first(&bus).await;
    let second = SystemInstance::new(bus.env());

    let tab = Request::NewTab {
        cwd: Some(PathBuf::from("/tmp/work")),
    };
    for request in [Request::NewWindow, tab, Request::NewTab { cwd: None }] {
        let claim = second.claim(&request).await.unwrap();
        assert!(matches!(claim, Claim::Forwarded), "{claim:?}");
        let heard = tokio::time::timeout(Duration::from_secs(5), primary.next()).await;
        assert_eq!(heard.unwrap(), Some(request));
    }
}

#[tokio::test]
async fn a_second_launch_passes_its_activation_token_on_and_an_empty_one_is_none() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = first(&bus).await;
    let second = SystemInstance::new(bus.env());

    let cases = [
        (Some("tok-1"), Some("tok-1".to_owned())),
        (Some(""), None),
        (None, None),
    ];
    for (sent, expected) in cases {
        let claim = second.claim_with(&Request::NewWindow, sent).await.unwrap();
        assert!(matches!(claim, Claim::Forwarded), "{claim:?}");
        let heard = tokio::time::timeout(Duration::from_secs(5), primary.next_handover()).await;
        let heard = heard.unwrap().expect("a handover");
        assert_eq!(heard.request, Request::NewWindow);
        assert_eq!(heard.token, expected);
    }
}

#[tokio::test]
async fn the_name_is_free_again_once_the_first_launch_is_gone() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    drop(first(&bus).await);
    // The bus releases the name when the connection closes; ask until it has.
    for _ in 0..50 {
        let claim = SystemInstance::new(bus.env())
            .claim(&Request::NewWindow)
            .await
            .unwrap();
        if matches!(claim, Claim::Primary(_)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the name was never released");
}

/// A run with words that are not text, as the system keeps them.
fn a_run() -> Request {
    Request::Run(RunRequest {
        argv: vec![
            OsString::from("claude"),
            OsString::from("--resume"),
            OsString::from_vec(vec![b'i', 0xff, b'd']),
        ],
        cwd: Some(PathBuf::from("/tmp/work")),
        env: vec![(OsString::from("TOOL_SESSION"), OsString::from("abc 123"))],
        ticket: None,
    })
}

#[tokio::test]
async fn a_run_from_temors_own_launch_is_heard_with_its_words_intact() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let instance = SystemInstance::new(bus.env());
    let Claim::Primary(mut primary) = instance.claim(&Request::NewWindow).await.unwrap() else {
        panic!("the first launch must own the name");
    };
    let launch = bus.connect().await;
    instance.vouch_for(launch.unique_name().expect("a unique name").as_str());
    SystemInstance::forward_for_tests(&launch, &a_run(), None)
        .await
        .expect("temor's own launch may run");
    let heard = tokio::time::timeout(Duration::from_secs(5), primary.next()).await;
    let Some(Request::Run(heard)) = heard.unwrap() else {
        panic!("a run");
    };
    assert!(heard.ticket.is_some(), "the launch waits on a ticket");
    assert_eq!(
        Request::Run(RunRequest {
            ticket: None,
            ..heard
        }),
        a_run()
    );
}

/// The waiting side of `temor -e`, on a vouched-for connection of the test's.
async fn waiting_launch(
    bus: &PrivateBus,
    instance: &SystemInstance,
) -> tokio::task::JoinHandle<Result<i32, temor_desktop::DesktopError>> {
    let launch = bus.connect().await;
    instance.vouch_for(launch.unique_name().expect("a unique name").as_str());
    let Request::Run(run) = a_run() else {
        unreachable!()
    };
    tokio::spawn(async move { SystemInstance::run_and_wait_for_tests(&launch, &run, None).await })
}

#[tokio::test]
async fn a_waiting_launch_exits_with_the_status_its_tab_closed_with() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let instance = SystemInstance::new(bus.env());
    let Claim::Primary(mut primary) = instance.claim(&Request::NewWindow).await.unwrap() else {
        panic!("the first launch must own the name");
    };
    let waiting = waiting_launch(&bus, &instance).await;
    let heard = tokio::time::timeout(Duration::from_secs(5), primary.next()).await;
    let Some(Request::Run(RunRequest {
        ticket: Some(ticket),
        ..
    })) = heard.unwrap()
    else {
        panic!("a run with a ticket");
    };
    // Another ticket's end is not this launch's.
    primary.run_ends().ended(RunTicket(ticket.0 + 100), 9);
    primary.run_ends().ended(ticket, 3);
    let status = tokio::time::timeout(Duration::from_secs(5), waiting).await;
    assert_eq!(status.unwrap().unwrap().unwrap(), 3);
}

#[tokio::test]
async fn a_waiting_launch_ends_as_hung_up_when_temor_goes_away() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let instance = SystemInstance::new(bus.env());
    let Claim::Primary(mut primary) = instance.claim(&Request::NewWindow).await.unwrap() else {
        panic!("the first launch must own the name");
    };
    let waiting = waiting_launch(&bus, &instance).await;
    let heard = tokio::time::timeout(Duration::from_secs(5), primary.next()).await;
    assert!(matches!(heard.unwrap(), Some(Request::Run(_))));
    drop(primary);
    let status = tokio::time::timeout(Duration::from_secs(5), waiting).await;
    assert_eq!(
        status.unwrap().unwrap().unwrap(),
        temor_desktop::CLOSED_WHILE_RUNNING
    );
}

#[tokio::test]
async fn a_run_from_anyone_else_is_refused_and_a_tab_is_still_theirs_to_ask() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let mut primary = first(&bus).await;
    // Any other program on the bus, a sandboxed app included.
    let intruder = bus.connect().await;
    let refused = SystemInstance::forward_for_tests(&intruder, &a_run(), None).await;
    let said = format!(
        "{:?}",
        refused.expect_err("an unknown caller must be refused")
    );
    assert!(said.contains("AccessDenied"), "{said}");
    // A temor the person built is an image they can write: its own launch is refused too.
    let built = SystemInstance::new(bus.env()).claim(&a_run()).await;
    assert!(built.is_err(), "{built:?}");
    // Nothing reached the window; a new tab still does.
    let tab = Request::NewTab { cwd: None };
    SystemInstance::forward_for_tests(&intruder, &tab, None)
        .await
        .expect("a tab is open to anyone");
    let heard = tokio::time::timeout(Duration::from_secs(5), primary.next()).await;
    assert_eq!(heard.unwrap(), Some(tab));
}

//! Single instance: the first launch owns the name, a second asks it to open a window or tab and
//! is told to exit.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use std::path::PathBuf;
use std::time::Duration;
use temor_desktop::{Claim, Primary, Request, SystemInstance};

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

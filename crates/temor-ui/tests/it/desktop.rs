//! Joining the desktop on a private session bus (never the person's): the first launch keeps its
//! window's link, and a second launch hands its request over and is told to end. The agent
//! layer, the dock and the Tailscale list are held by temor-desktop's own bus tests.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use std::time::Duration;

use temor_client::desktop::port::Ask;
use temor_desktop::{DesktopEnv, Request};
use temor_testkit::PrivateBus;
use temor_ui::app::desktop::{Joined, join};

#[test]
fn a_second_launch_is_handed_over_and_the_first_window_hears_its_request() {
    let Some(bus) = PrivateBus::start("desktop") else {
        return;
    };
    let env = DesktopEnv::at(bus.address().to_owned());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");

    let first = join(runtime.handle(), env.clone(), &Request::NewWindow, None);
    let Joined::First { desktop, trouble } = first else {
        panic!("the first launch must own the name: {first:?}");
    };
    assert_eq!(trouble, None);
    let mut asks = desktop
        .link
        .expect("a link to the window")
        .take_asks()
        .expect("the window's asks");

    let tab = Request::NewTab {
        cwd: Some("/tmp/work".into()),
    };
    for request in [Request::NewWindow, tab] {
        let second = join(runtime.handle(), env.clone(), &request, None);
        assert!(matches!(second, Joined::Handed), "{second:?}");
        let heard = runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), asks.recv()).await });
        match heard {
            Ok(Some(Ask::Launch(got, _))) => assert_eq!(got, request),
            other => panic!("the first window should hear {request:?}, got {other:?}"),
        }
    }
}

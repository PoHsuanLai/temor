//! With no bus, a stopped Tailscale, or a build without the desktop services, the window can
//! call the same API and gets "not there", never a failure to show.

use temor_desktop::{DesktopEnv, SystemDock, SystemTailnet};

#[tokio::test]
async fn without_a_bus_there_are_no_computers_and_no_dock() {
    let env = DesktopEnv::absent();
    let tailnet = SystemTailnet::connect(&env).await.unwrap();
    assert!(tailnet.hosts().await.unwrap().is_empty());
    assert_eq!(tailnet.changes().await.unwrap().next().await, None);
    assert!(SystemDock::connect(&env).await.is_err());
}

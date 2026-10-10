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

#[test]
fn with_no_agent_layer_nothing_is_listed_and_nothing_restores() {
    use temor_desktop::{CheckpointPort, NoCheckpoints, PlanDigest, PointId, Rewind, SessionKey};
    let session = SessionKey::new("s-1").unwrap();
    assert!(NoCheckpoints.list(&session).unwrap().rows.is_empty());
    assert!(NoCheckpoints.plan(&session, PointId(1)).is_err());
    assert!(
        NoCheckpoints
            .watch("/work", "claude", Rewind::Agent)
            .is_err()
    );
    let digest = PlanDigest("d".to_owned());
    assert!(
        NoCheckpoints
            .restore(&session, PointId(1), &digest)
            .is_err()
    );
}

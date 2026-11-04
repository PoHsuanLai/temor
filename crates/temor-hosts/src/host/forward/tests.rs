use super::{ForwardFault, ForwardIn, ForwardOut, Forwards, PortOffer, forward_step, offers};
use temor_ssh::{Forward, Port, RemoteHost};
use temor_testkit::run_steps;

fn port(n: u16) -> Port {
    Port::parse(&n.to_string()).expect("port")
}

fn fwd(local: u16, remote: u16) -> Forward {
    Forward {
        bind: None,
        local: port(local),
        remote_host: RemoteHost::localhost(),
        remote: port(remote),
    }
}

fn run(from: Forwards, inputs: &[ForwardIn]) -> (Forwards, Vec<ForwardOut>) {
    run_steps(from, inputs.iter().copied(), forward_step)
}

#[test]
fn a_forward_is_opened_once_used_again_and_closed() {
    let ask = ForwardIn::Requested {
        remote: port(3000),
        local: port(3000),
    };
    let (state, outs) = run(
        Forwards::default(),
        &[
            ask,
            // Asked again while it is still opening: nothing new to do.
            ForwardIn::Requested {
                remote: port(3000),
                local: port(4000),
            },
            ForwardIn::Opened { remote: port(3000) },
            // Asked again once open: open it again where it already is.
            ForwardIn::Requested {
                remote: port(3000),
                local: port(4001),
            },
        ],
    );
    assert_eq!(
        outs,
        vec![
            ForwardOut::Open(fwd(3000, 3000)),
            ForwardOut::Ready {
                remote: port(3000),
                local: port(3000)
            },
            ForwardOut::Ready {
                remote: port(3000),
                local: port(3000)
            },
        ]
    );
    assert_eq!(state.local_of(port(3000)), Some(port(3000)));
    let (state, outs) = forward_step(state, ForwardIn::Cancelled { remote: port(3000) });
    assert_eq!(outs, vec![ForwardOut::Close(fwd(3000, 3000))]);
    assert_eq!(state, Forwards::default());
}

#[test]
fn a_failed_forward_leaves_nothing_behind_and_late_replies_are_ignored() {
    let (state, outs) = run(
        Forwards::default(),
        &[
            ForwardIn::Requested {
                remote: port(80),
                local: port(8080),
            },
            ForwardIn::Failed {
                remote: port(80),
                fault: ForwardFault::LocalPortTaken,
            },
            // A reply that arrives after the failure was handled, or after a cancel.
            ForwardIn::Opened { remote: port(80) },
            ForwardIn::Cancelled { remote: port(80) },
        ],
    );
    assert_eq!(
        outs,
        vec![
            ForwardOut::Open(fwd(8080, 80)),
            ForwardOut::Report {
                remote: port(80),
                fault: ForwardFault::LocalPortTaken
            }
        ]
    );
    assert_eq!(state, Forwards::default());
}

#[test]
fn losing_the_connection_ends_every_forward_and_lists_them() {
    let (state, outs) = run(
        Forwards::default(),
        &[
            ForwardIn::Requested {
                remote: port(3000),
                local: port(3000),
            },
            ForwardIn::Opened { remote: port(3000) },
            ForwardIn::Requested {
                remote: port(5432),
                local: port(15432),
            },
            ForwardIn::ConnectionLost,
            ForwardIn::ConnectionLost,
        ],
    );
    assert_eq!(
        outs.last(),
        Some(&ForwardOut::Lost(vec![port(3000), port(5432)]))
    );
    assert_eq!(outs.len(), 4, "the second loss says nothing");
    assert_eq!(state, Forwards::default());
}

#[test]
fn a_tailnet_port_is_offered_directly_before_a_forward() {
    let machine = temor_ssh::merge_hosts(
        &temor_ssh::HostList::default(),
        &[temor_ssh::TailnetHost {
            node: temor_ssh::NodeId::new("n1").expect("node"),
            name: "box".to_owned(),
            dns: "box.tail1.ts.net".to_owned(),
            addresses: vec![],
            online: true,
            last_seen: None,
            tailscale_ssh: true,
            owner: temor_ssh::TailnetOwner::Mine,
        }],
    );
    let direct = machine[0].direct_host();
    let got = offers(direct.clone(), port(3000));
    assert_eq!(got.len(), 2);
    assert!(matches!(&got[0], PortOffer::OpenDirect { host, .. }
        if host.authority(port(3000)) == "box.tail1.ts.net:3000"));
    assert_eq!(got[1], PortOffer::Forward { remote: port(3000) });
    assert_eq!(
        offers(None, port(3000)),
        vec![PortOffer::Forward { remote: port(3000) }]
    );
}

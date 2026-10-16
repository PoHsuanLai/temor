//! `agent.adapter` through a real plugin: events in, stop on request.

use std::cell::{Cell, RefCell};

use temor_plugins::message::Phase;
use temor_plugins::{Cancel, PluginError};

use crate::support::{ALL, Host, launcher};

#[test]
fn an_adapter_that_finishes_reports_its_states_in_order() {
    let host = Host::new();
    host.install("a", &[], ALL);
    let events = RefCell::new(Vec::new());
    let done = launcher().adapt(
        &host.plugin("a"),
        "claude",
        "/finish",
        &|| Cancel::Go,
        &mut |phase, text| events.borrow_mut().push((phase, text)),
    );
    assert_eq!(done, Ok(()));
    assert_eq!(
        events.into_inner(),
        vec![
            (Phase::Working, "thinking".to_owned()),
            (Phase::Done, "all done".to_owned())
        ]
    );
}

#[test]
fn cancelling_stops_an_adapter_that_would_go_on() {
    let host = Host::new();
    host.install("a", &[], ALL);
    let stop = Cell::new(Cancel::Go);
    let done = launcher().adapt(
        &host.plugin("a"),
        "claude",
        "/work",
        &|| stop.get(),
        &mut |phase, _| {
            if phase == Phase::WaitingPermission {
                stop.set(Cancel::Stop);
            }
        },
    );
    assert!(
        matches!(done, Err(PluginError::Cancelled { .. })),
        "{done:?}"
    );
}

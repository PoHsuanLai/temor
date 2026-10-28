use super::{
    CacheDir, Grant, HelperIn, HelperOut, HelperState, HostArch, HostOs, Link, LinkIn, LinkOut,
    LinkParams, Probe, helper_step, link_step,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use temor_core::vocab::ids::Attempt;
use temor_core::vocab::text::HelperHash;

fn detached(since: u64, attempt: u32, retry_at: u64) -> Link {
    Link::Detached {
        since: Stamp(since),
        attempt: Attempt(attempt),
        retry_at: Stamp(retry_at),
    }
}
fn reattaching(since: u64, attempt: u32) -> Link {
    Link::Reattaching {
        since: Stamp(since),
        attempt: Attempt(attempt),
    }
}

/// Name, state before, input, time, state after, outputs.
type LinkCase = (&'static str, Link, LinkIn, u64, Link, Vec<LinkOut>);
/// Name, state before, input, remembered consent, state after, outputs.
type HelperCase = (
    &'static str,
    HelperState,
    HelperIn,
    Grant,
    HelperState,
    Vec<HelperOut>,
);

#[test]
fn a_link_reconnects_with_backoff_and_reattaches() {
    let params = LinkParams {
        first_wait_ms: 100,
        longest_wait_ms: 300,
        most_attempts: Attempt(3),
    };
    #[rustfmt::skip]
    let cases: Vec<LinkCase> = vec![
        ("hello attaches", Link::Connecting, LinkIn::Hello, 0, Link::Attached, vec![LinkOut::Reattached]),
        ("a first connect failing is lost", Link::Connecting, LinkIn::SshExited, 0, Link::Lost, vec![LinkOut::GaveUp]),
        ("a drop detaches and waits", Link::Attached, LinkIn::SshExited, 50, detached(50, 0, 150), vec![LinkOut::ShowDetached]),
        ("a network change probes", Link::Attached, LinkIn::NetworkChanged, 0, Link::Attached, vec![LinkOut::Probe]),
        ("a wake probes", Link::Attached, LinkIn::Woke, 0, Link::Attached, vec![LinkOut::Probe]),
        ("an early timer does nothing", detached(50, 0, 150), LinkIn::Elapsed, 149, detached(50, 0, 150), vec![]),
        ("the timer reconnects", detached(50, 0, 150), LinkIn::Elapsed, 150, reattaching(50, 1), vec![LinkOut::Reconnect]),
        ("the network coming back retries at once", detached(50, 2, 900), LinkIn::NetworkChanged, 60, reattaching(50, 1), vec![LinkOut::Reconnect]),
        ("a failed try waits twice as long", reattaching(50, 1), LinkIn::SshExited, 200, detached(50, 1, 400), vec![]),
        ("the wait is capped", reattaching(50, 2), LinkIn::SshExited, 200, detached(50, 2, 500), vec![]),
        ("too many tries gives up", reattaching(50, 3), LinkIn::SshExited, 200, Link::Lost, vec![LinkOut::GaveUp]),
        ("hello reattaches", reattaching(50, 2), LinkIn::Hello, 300, Link::Attached, vec![LinkOut::Reattached]),
        ("a retry from lost", Link::Lost, LinkIn::Retry, 500, reattaching(500, 1), vec![LinkOut::Reconnect]),
        ("a wake from lost", Link::Lost, LinkIn::Woke, 500, reattaching(500, 1), vec![LinkOut::Reconnect]),
    ];
    for (name, from, input, at, want, outs) in cases {
        let (link, out) = link_step(from, input, Stamp(at), &params);
        assert_eq!(link, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}

#[test]
fn a_dropped_link_runs_to_lost_on_its_timers_alone() {
    let params = LinkParams {
        first_wait_ms: 100,
        longest_wait_ms: 1000,
        most_attempts: Attempt(2),
    };
    let (mut link, mut at) = (
        link_step(Link::Attached, LinkIn::SshExited, Stamp(0), &params).0,
        Stamp(0),
    );
    let mut tries = 0;
    while let Some(due) = link.wake() {
        at = due;
        let (next, _) = link.step(LinkIn::Elapsed, at, &params, &());
        assert!(matches!(next, Link::Reattaching { .. }), "{next:?}");
        tries += 1;
        link = link_step(next, LinkIn::SshExited, at, &params).0;
    }
    assert_eq!((link, tries, at), (Link::Lost, 2, Stamp(300)));
}

fn hash(text: &str) -> HelperHash {
    HelperHash(text.into())
}
fn probe(os: HostOs, arch: HostArch, cache: CacheDir, installed: Option<&str>) -> HelperIn {
    HelperIn::Probed(Probe {
        os,
        arch,
        cache,
        installed: installed.map(hash),
    })
}

#[test]
fn the_helper_installs_only_where_it_fits_and_is_allowed() {
    use CacheDir::{Unusable, Usable};
    use HostArch::{Aarch64, X86_64};
    let fits = |installed| probe(HostOs::Linux, X86_64, Usable, installed);
    #[rustfmt::skip]
    let cases: Vec<HelperCase> = vec![
        ("another system stays plain", HelperState::Absent, probe(HostOs::Other, X86_64, Usable, None), Grant::Allowed,
            HelperState::Unsupported, vec![HelperOut::StayPlain]),
        ("another processor stays plain", HelperState::Absent, probe(HostOs::Linux, HostArch::Other, Usable, None), Grant::Allowed,
            HelperState::Unsupported, vec![HelperOut::StayPlain]),
        ("a noexec cache stays plain", HelperState::Absent, probe(HostOs::Linux, Aarch64, Unusable, None), Grant::Allowed,
            HelperState::Unsupported, vec![HelperOut::StayPlain]),
        ("unasked is offered", HelperState::Absent, fits(None), Grant::Ask, HelperState::Offered, vec![HelperOut::AskPerson]),
        ("refused before stays plain", HelperState::Absent, fits(None), Grant::Refused, HelperState::Refused, vec![HelperOut::StayPlain]),
        ("allowed uploads", HelperState::Absent, fits(None), Grant::Allowed, HelperState::Uploading, vec![HelperOut::Upload(hash("new"))]),
        ("the wanted build is used as it is", HelperState::Absent, fits(Some("new")), Grant::Allowed,
            HelperState::Ready(hash("new")), vec![HelperOut::UseHelper(hash("new"))]),
        ("an old build is replaced", HelperState::Ready(hash("old")), fits(Some("old")), Grant::Allowed,
            HelperState::Uploading, vec![HelperOut::RemoveFromHost(hash("old")), HelperOut::Upload(hash("new"))]),
        ("yes uploads and is remembered", HelperState::Offered, HelperIn::Answered(Grant::Allowed), Grant::Ask,
            HelperState::Uploading, vec![HelperOut::Remember(Grant::Allowed), HelperOut::Upload(hash("new"))]),
        ("no is remembered", HelperState::Offered, HelperIn::Answered(Grant::Refused), Grant::Ask,
            HelperState::Refused, vec![HelperOut::Remember(Grant::Refused), HelperOut::StayPlain]),
        ("a matching hash is ready", HelperState::Uploading, HelperIn::Uploaded(hash("new")), Grant::Allowed,
            HelperState::Ready(hash("new")), vec![HelperOut::UseHelper(hash("new"))]),
        ("a hash mismatch is removed", HelperState::Uploading, HelperIn::Uploaded(hash("bad")), Grant::Allowed,
            HelperState::Unsupported, vec![HelperOut::RemoveFromHost(hash("bad")), HelperOut::StayPlain]),
        ("a failed upload stays plain", HelperState::Uploading, HelperIn::UploadFailed, Grant::Allowed,
            HelperState::Unsupported, vec![HelperOut::StayPlain]),
    ];
    for (name, from, input, consent, want, outs) in cases {
        let (state, out) = helper_step(from, input, &hash("new"), consent);
        assert_eq!(state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}

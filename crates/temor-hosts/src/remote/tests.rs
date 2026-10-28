use super::{
    CacheDir, Grant, HelperIn, HelperOut, HelperState, HostArch, HostOs, Probe, helper_step,
};
use temor_core::vocab::text::HelperHash;

type HelperCase = (
    &'static str,
    HelperState,
    HelperIn,
    Grant,
    HelperState,
    Vec<HelperOut>,
);

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

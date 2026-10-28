use super::*;

const WRITES_ON: ApprovalMode = ApprovalMode {
    sandbox: SandboxMode::On,
    asks: AsksLevel::Writes,
};

#[test]
fn the_line_comes_from_launch_and_gate_and_never_guesses() {
    use ApprovalLine::{Gated, NotGated};
    use GateFact::{NoSession, Session};
    use LaunchFact::{Sandboxed, Unknown, Unsandboxed};
    #[rustfmt::skip]
    let cases = [
        (Sandboxed, Session(AsksLevel::Writes), Gated(WRITES_ON), "Sandbox: On · Asks: Writes"),
        (Unsandboxed, Session(AsksLevel::Everything), Gated(ApprovalMode { sandbox: SandboxMode::Off, asks: AsksLevel::Everything }), "Sandbox: Off · Asks: Everything"),
        (Sandboxed, NoSession, NotGated { sandbox: Some(SandboxMode::On) }, "Not gated"),
        (Unsandboxed, NoSession, NotGated { sandbox: Some(SandboxMode::Off) }, "Not gated"),
        (Unknown, Session(AsksLevel::Nothing), NotGated { sandbox: None }, "Not gated"),
    ];
    for (launch, gate, line, text) in cases {
        let got = approval_line(launch, gate);
        assert_eq!(
            (got, got.text().as_str()),
            (line, text),
            "{launch:?} {gate:?}"
        );
    }
    assert!(
        tooltip(&NotGated {
            sandbox: Some(SandboxMode::On)
        })
        .contains("sandbox")
    );
}

#[test]
fn a_change_is_applied_on_request_and_shown_only_when_read_back() {
    let read = |asks| ApprovalIn::Read(LaunchFact::Sandboxed, GateFact::Session(asks));
    let (s, out) = approval_step(ApprovalState::UNREAD, read(AsksLevel::Writes));
    assert!(out.is_empty() && s.line == ApprovalLine::Gated(WRITES_ON));

    let want = ModeChange::Asks(AsksLevel::Nothing);
    let (s, out) = approval_step(s, ApprovalIn::Choose(want));
    assert_eq!(out, vec![ApprovalOut::Apply(want)]);
    assert_eq!(
        s.line,
        ApprovalLine::Gated(WRITES_ON),
        "the line waits for the gate"
    );
    assert_eq!(s.requested, Some(want));

    let (s, _) = approval_step(s, read(AsksLevel::Nothing));
    assert_eq!(s.requested, None);
    let (_, out) = approval_step(s, ApprovalIn::Choose(want));
    assert!(out.is_empty(), "choosing what is already so does nothing");

    assert!(ModeChange::Sandbox(SandboxMode::On).restarts_agent());
    assert!(!want.restarts_agent());
    assert_eq!(new_agent_mode(WRITES_ON), WRITES_ON);
}

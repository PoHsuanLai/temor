//! A pane saved with the session it had on a host (PLAN §9b, D18): after a restart it asks for
//! that session again, if the host still holds it.

use temor_wire::ids::SessionId;

/// What a restored pane does about the session it was saved with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adoption {
    /// The host still has it: attach to it and show its screen.
    Adopt,
    /// It is gone (ended, or the helper was restarted): open a new one.
    OpenNew,
}

/// Whether `saved` is among the sessions the host `held` just now.
pub fn adoption(saved: SessionId, held: &[SessionId]) -> Adoption {
    match held.contains(&saved) {
        true => Adoption::Adopt,
        false => Adoption::OpenNew,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_session_is_adopted_only_while_the_host_still_has_it() {
        let held = [SessionId(2), SessionId(5)];
        let cases = [
            (SessionId(5), &held[..], Adoption::Adopt),
            (SessionId(3), &held[..], Adoption::OpenNew),
            (SessionId(5), &[][..], Adoption::OpenNew),
        ];
        for (saved, held, want) in cases {
            assert_eq!(adoption(saved, held), want, "{saved:?} among {held:?}");
        }
    }
}

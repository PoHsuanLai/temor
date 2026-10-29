//! The answers once kept as a host's `HostConsent`, carried into the consent book once, so
//! what a person already answered is not asked again.

use super::model::{ConsentBook, Permit, Who};
use super::step::{ConsentIn, consent_step};
use crate::remote::{Grant, HostConsent};

/// `book` with the old answers in `old` filled in for the machine `who` names. An answer the
/// book already has wins, and an open (`Ask`) old answer carries nothing. Running it again
/// changes nothing.
pub fn carry_host_consent(book: ConsentBook, who: &Who, old: &HostConsent) -> ConsentBook {
    let permits = [
        (Permit::Clipboard, old.clipboard),
        (Permit::Transfers, old.transfers),
        (Permit::Notifications, old.notifications),
    ];
    let book = permits.into_iter().fold(book, |book, (permit, grant)| {
        match (book.grants(who).permit(permit), grant) {
            (Grant::Ask, Grant::Allowed | Grant::Refused) => {
                let input = ConsentIn::PermitAnswered {
                    who: who.clone(),
                    permit,
                    grant,
                };
                consent_step(book, input).0
            }
            _ => book,
        }
    });
    match (book.grants(who).helper, old.helper) {
        (Grant::Ask, Grant::Allowed | Grant::Refused) => {
            let input = ConsentIn::HelperAnswered {
                who: who.clone(),
                grant: old.helper,
            };
            consent_step(book, input).0
        }
        _ => book,
    }
}

//! The helper question's answers live in the consent book beside the bootstrap's (PLAN §9b,
//! "Consent per host"), so a renamed host or a second alias is not asked again. The old
//! `remote.json` is read once and its answers carried over.

use std::collections::BTreeMap;

use temor_client::model::host::consent::{ConsentBook, ConsentIn, Who, consent_step};
use temor_client::model::remote::Grant;
use temor_client::ssh::HostAlias;

use super::model::Hub;

/// `book` with the old answers in `old` (by the name each host was opened by) filled in. An
/// answer the book already has wins, and an open (`Ask`) old answer carries nothing.
pub(super) fn carry_over(book: ConsentBook, old: &BTreeMap<String, Grant>) -> ConsentBook {
    old.iter()
        .filter(|(_, grant)| **grant != Grant::Ask)
        .fold(book, |book, (alias, grant)| {
            let who = Who {
                key: None,
                alias: alias.clone(),
            };
            let held = book.grants(&who).helper;
            match held {
                Grant::Ask => {
                    let input = ConsentIn::HelperAnswered { who, grant: *grant };
                    consent_step(book, input).0
                }
                _ => book,
            }
        })
}

impl Hub {
    /// What the person answered about the helper on `alias`.
    pub(super) fn helper_grant(&self, alias: &HostAlias) -> Grant {
        self.ssh.book.grants(&self.who_of(alias)).helper
    }

    /// Remembers the person's answer about the helper on `alias`.
    pub(super) fn remember_helper(&mut self, alias: &HostAlias, grant: Grant) {
        let who = self.who_of(alias);
        self.step_book(ConsentIn::HelperAnswered { who, grant });
    }

    /// Moves the old `remote.json` answers into the book, once, after the book was loaded.
    pub(super) fn remote_carry_over(&mut self) {
        let old = std::mem::take(&mut self.remote.legacy);
        if old.is_empty() {
            return;
        }
        let book = std::mem::take(&mut self.ssh.book);
        let next = carry_over(book.clone(), &old);
        if next != book {
            self.pace.changed();
        }
        self.ssh.book = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(pairs: &[(&str, Grant)]) -> BTreeMap<String, Grant> {
        pairs.iter().map(|(a, g)| ((*a).to_owned(), *g)).collect()
    }

    fn helper_of(book: &ConsentBook, alias: &str) -> Grant {
        let who = Who {
            key: None,
            alias: alias.to_owned(),
        };
        book.grants(&who).helper
    }

    #[test]
    fn old_answers_carry_over_unless_the_book_already_answered_or_the_old_one_was_open() {
        let kept = consent_step(
            ConsentBook::default(),
            ConsentIn::HelperAnswered {
                who: Who {
                    key: None,
                    alias: "box".to_owned(),
                },
                grant: Grant::Allowed,
            },
        )
        .0;
        let old = answers(&[
            ("prod", Grant::Allowed),
            ("box", Grant::Refused),
            ("new", Grant::Ask),
        ]);
        let cases = [
            ("prod", Grant::Allowed, "carried over"),
            ("box", Grant::Allowed, "the book's own answer wins"),
            ("new", Grant::Ask, "an open answer carries nothing"),
            ("other", Grant::Ask, "an unlisted host is still asked"),
        ];
        let got = carry_over(kept.clone(), &old);
        for (alias, want, why) in cases {
            assert_eq!(helper_of(&got, alias), want, "{alias}: {why}");
        }
        assert_eq!(
            carry_over(got.clone(), &old),
            got,
            "a second pass changes nothing"
        );
        assert_eq!(carry_over(kept.clone(), &BTreeMap::new()), kept);
    }
}

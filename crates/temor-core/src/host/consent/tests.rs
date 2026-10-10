use super::{
    BootstrapAction as Action, BootstrapGrants, ConsentBook, ConsentIn, ConsentOut, Gate, RunMode,
    Who, consent_step, gate,
};
use crate::remote::Grant;
use temor_ssh::{HostKey, TerminfoPolicy};

const FP: &str = "SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s";
const OTHER_FP: &str = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn who(alias: &str, fp: Option<&str>, node: Option<&str>) -> Who {
    Who {
        key: HostKey::from_parts(fp, node),
        alias: alias.to_owned(),
    }
}

fn answer(book: ConsentBook, who: &Who, action: Action, grant: Grant) -> ConsentBook {
    let input = ConsentIn::Answered {
        who: who.clone(),
        action,
        grant,
    };
    consent_step(book, input).0
}

fn identify(book: ConsentBook, who: &Who) -> ConsentBook {
    consent_step(book, ConsentIn::Identified { who: who.clone() }).0
}

const INSTALL: Gate = Gate::Run(RunMode::Bootstrap(TerminfoPolicy::Install));
const SKIP: Gate = Gate::Run(RunMode::Bootstrap(TerminfoPolicy::Skip));

#[test]
fn a_host_is_asked_one_question_at_a_time_and_remembered() {
    let prod = who("prod", Some(FP), None);
    let mut book = ConsentBook::default();
    assert_eq!(gate(&book, &prod), Gate::Ask(Action::Integrate));
    book = answer(book, &prod, Action::Integrate, Grant::Allowed);
    assert_eq!(gate(&book, &prod), Gate::Ask(Action::Terminfo));
    book = answer(book, &prod, Action::Terminfo, Grant::Refused);
    assert_eq!(gate(&book, &prod), SKIP);
    book = answer(book, &prod, Action::Terminfo, Grant::Allowed);
    assert_eq!(gate(&book, &prod), INSTALL);
    book = answer(book, &prod, Action::Terminfo, Grant::Ask);
    assert_eq!(gate(&book, &prod), Gate::Ask(Action::Terminfo), "forgotten");
    // Refusing the setup runs a plain session and never asks about the terminfo file.
    book = answer(book, &prod, Action::Integrate, Grant::Refused);
    assert_eq!(gate(&book, &prod), Gate::Run(RunMode::Plain));
}

#[test]
fn answers_follow_the_machine_not_its_name() {
    // Name, answers recorded for, asked as, expected gate.
    let cases = [
        (
            "another alias for the same machine",
            who("prod", Some(FP), None),
            who("prod-2", Some(FP), None),
            INSTALL,
        ),
        (
            "an alias reused by a different machine",
            who("prod", Some(FP), None),
            who("prod", Some(OTHER_FP), None),
            Gate::Ask(Action::Integrate),
        ),
        (
            "the same alias before a key is known",
            who("prod", Some(FP), None),
            who("prod", None, None),
            INSTALL,
        ),
    ];
    for (name, recorded, asked, want) in cases {
        let mut book = ConsentBook::default();
        book = answer(book, &recorded, Action::Integrate, Grant::Allowed);
        book = answer(book, &recorded, Action::Terminfo, Grant::Allowed);
        assert_eq!(gate(&book, &asked), want, "{name}");
    }
}

#[test]
fn answers_given_by_alias_join_the_machine_once_a_connection_proves_it() {
    let typed = who("prod", None, None);
    let mut book = answer(
        ConsentBook::default(),
        &typed,
        Action::Integrate,
        Grant::Allowed,
    );
    book = answer(book, &typed, Action::Terminfo, Grant::Allowed);
    assert_eq!(gate(&book, &typed), INSTALL);

    let proven = who("prod", Some(FP), None);
    let (book, outs) = consent_step(
        book,
        ConsentIn::Identified {
            who: proven.clone(),
        },
    );
    assert_eq!(outs, vec![ConsentOut::Persist]);
    // Reached by another name later (a rename in ssh_config), it is the same machine.
    assert_eq!(gate(&book, &who("renamed", Some(FP), None)), INSTALL);
    // Identifying again changes nothing and asks for no save.
    let (again, outs) = consent_step(book.clone(), ConsentIn::Identified { who: proven });
    assert_eq!((again, outs), (book, Vec::new()));
}

#[test]
fn a_tailnet_node_id_keeps_the_answers_through_a_rename_and_a_new_host_key() {
    let by_key = who("box", Some(FP), None);
    let mut book = answer(
        ConsentBook::default(),
        &by_key,
        Action::Integrate,
        Grant::Allowed,
    );
    book = answer(book, &by_key, Action::Terminfo, Grant::Refused);
    // The tailnet shows that this machine is node n1.
    book = identify(book, &who("box", Some(FP), Some("n1")));
    // Before any connection the window only has the node id.
    assert_eq!(gate(&book, &who("anything", None, Some("n1"))), SKIP);
    // Later the host key changes; the node id still finds the answers.
    assert_eq!(
        gate(&book, &who("anything", Some(OTHER_FP), Some("n1"))),
        SKIP
    );
    // A different node with the same key (a cloned image) starts fresh.
    assert_eq!(
        gate(&book, &who("clone", Some(FP), Some("n2"))),
        Gate::Ask(Action::Integrate)
    );
}

#[test]
fn two_records_for_one_machine_fold_into_one_and_the_earlier_answer_wins() {
    let by_node = who("a", None, Some("n1"));
    let by_key = who("b", Some(FP), None);
    let mut book = answer(
        ConsentBook::default(),
        &by_node,
        Action::Integrate,
        Grant::Allowed,
    );
    book = answer(book, &by_key, Action::Integrate, Grant::Refused);
    book = answer(book, &by_key, Action::Terminfo, Grant::Allowed);
    assert_eq!(book.records().len(), 2);
    book = identify(book, &who("b", Some(FP), Some("n1")));
    assert_eq!(book.records().len(), 1);
    assert_eq!(
        book.grants(&by_node),
        BootstrapGrants {
            integrate: Grant::Allowed,
            terminfo: Grant::Allowed,
            ..BootstrapGrants::default()
        }
    );
}

#[test]
fn the_book_is_stored_as_a_plain_list() {
    let prod = who("prod", Some(FP), Some("n1"));
    let book = answer(
        ConsentBook::default(),
        &prod,
        Action::Integrate,
        Grant::Allowed,
    );
    let json = serde_json::to_string(&book).expect("encode");
    assert_eq!(
        json,
        format!(
            r#"[{{"fingerprint":"{FP}","node":"n1","aliases":["prod"],"grants":{{"integrate":"allowed","terminfo":"ask"}}}}]"#
        )
    );
    assert_eq!(
        serde_json::from_str::<ConsentBook>(&json).expect("decode"),
        book
    );
    // A record written by an older build, with fields missing, still reads.
    let old: ConsentBook = serde_json::from_str(r#"[{"aliases":["x"]}]"#).expect("decode");
    assert_eq!(
        gate(&old, &who("x", None, None)),
        Gate::Ask(Action::Integrate)
    );
}

#[test]
fn the_helper_answer_is_kept_per_machine_beside_the_others_and_stays_out_of_the_file_while_open() {
    let prod = who("prod", Some(FP), Some("n1"));
    let alias_only = who("prod", None, None);
    let mut book = answer(
        ConsentBook::default(),
        &prod,
        Action::Integrate,
        Grant::Allowed,
    );
    assert_eq!(book.grants(&prod).helper, Grant::Ask);
    let helper = |book, who: &Who, grant| {
        consent_step(
            book,
            ConsentIn::HelperAnswered {
                who: who.clone(),
                grant,
            },
        )
    };
    let (next, outs) = helper(book.clone(), &alias_only, Grant::Allowed);
    assert_eq!(outs, vec![ConsentOut::Persist]);
    book = next;
    let kept = book.grants(&prod);
    assert_eq!(
        (kept.integrate, kept.helper),
        (Grant::Allowed, Grant::Allowed)
    );
    assert!(
        serde_json::to_string(&book)
            .expect("encode")
            .contains(r#""helper":"allowed""#)
    );
    let (same, outs) = helper(book.clone(), &prod, Grant::Allowed);
    assert!(
        outs.is_empty() && same == book,
        "the same answer changes nothing"
    );
    let (forgotten, _) = helper(book, &prod, Grant::Ask);
    assert!(
        !serde_json::to_string(&forgotten)
            .expect("encode")
            .contains("helper")
    );
}

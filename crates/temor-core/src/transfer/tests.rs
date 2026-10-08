use super::{Origin, Transfer, TransferIn, TransferOut, offer, transfer_step};
use crate::remote::Grant;
use crate::vocab::ids::{Bytes, TabId};
use crate::vocab::text::HostName;

#[test]
fn an_offer_is_accepted_refused_or_asked_by_origin_and_consent() {
    let remote = Origin::Remote {
        host: HostName("prod".into()),
        tab: TabId(1),
    };
    let local = Origin::Local {
        tab: TabId(1),
        block: None,
    };
    let receiving = Transfer::Receiving { received: Bytes(0) };
    #[rustfmt::skip]
    let cases: Vec<(&str, Origin, Grant, Transfer, Vec<TransferOut>)> = vec![
        ("local is accepted", local, Grant::Refused, receiving, vec![TransferOut::Accept]),
        ("a pipe is accepted", Origin::Pipe, Grant::Ask, receiving, vec![TransferOut::Accept]),
        ("an allowed host is accepted", remote.clone(), Grant::Allowed, receiving, vec![TransferOut::Accept]),
        ("a refused host is refused", remote.clone(), Grant::Refused, Transfer::Refused, vec![TransferOut::Refuse]),
        ("an unasked host asks", remote, Grant::Ask, Transfer::Offered, vec![TransferOut::AskPerson]),
    ];
    for (name, origin, consent, want, outs) in cases {
        assert_eq!(offer(&origin, consent), (want, outs), "{name}");
    }
}

#[test]
fn a_transfer_runs_to_the_shelf_or_breaks() {
    let receiving = |n| Transfer::Receiving { received: Bytes(n) };
    #[rustfmt::skip]
    let cases: Vec<(&str, Transfer, TransferIn, Transfer, Vec<TransferOut>)> = vec![
        ("yes is remembered and accepted", Transfer::Offered, TransferIn::Answered(Grant::Allowed), receiving(0),
            vec![TransferOut::Remember(Grant::Allowed), TransferOut::Accept]),
        ("no is remembered and refused", Transfer::Offered, TransferIn::Answered(Grant::Refused), Transfer::Refused,
            vec![TransferOut::Remember(Grant::Refused), TransferOut::Refuse]),
        ("closing the question refuses once", Transfer::Offered, TransferIn::Cancelled, Transfer::Refused, vec![TransferOut::Refuse]),
        ("data before an answer is ignored", Transfer::Offered, TransferIn::Chunk(Bytes(5)), Transfer::Offered, vec![]),
        ("chunks add up", receiving(10), TransferIn::Chunk(Bytes(5)), receiving(15), vec![]),
        ("chunks saturate", receiving(u64::MAX), TransferIn::Chunk(Bytes(5)), receiving(u64::MAX), vec![]),
        ("complete is shelved", receiving(15), TransferIn::Completed, Transfer::Saved, vec![TransferOut::Shelved]),
        ("a bad frame breaks it", receiving(15), TransferIn::Failed, Transfer::Broken, vec![TransferOut::Discard]),
        ("cancelling breaks it", receiving(15), TransferIn::Cancelled, Transfer::Broken, vec![TransferOut::Discard]),
        ("nothing after saved", Transfer::Saved, TransferIn::Failed, Transfer::Saved, vec![]),
    ];
    for (name, from, input, want, outs) in cases {
        assert_eq!(transfer_step(from, input), (want, outs), "{name}");
    }
}

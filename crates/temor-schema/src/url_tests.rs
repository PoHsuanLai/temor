use proptest::prelude::*;

use super::*;

const ID: SessionId = SessionId::from_bits(0x7f3a_0000_0000_00ff);

fn local(path: &str) -> TemorUrl {
    TemorUrl::Location(Location {
        host: Host::Local,
        path: path.to_owned(),
        within: None,
    })
}

#[test]
fn each_form_is_written_and_read_back() {
    let in_session = TemorUrl::Location(Location {
        host: Host::Named("box".to_owned()),
        path: "/srv/app/".to_owned(),
        within: Some(Within {
            session: ID,
            block: Some(BlockId(12)),
        }),
    });
    let table = [
        (local("/Users/po/ml/"), "temor://local/Users/po/ml/"),
        (in_session, "temor://box/srv/app/#7f3a0000000000ff/12"),
        (TemorUrl::History(HistoryScope::All), "temor://history"),
        (
            TemorUrl::History(HistoryScope::Session(ID)),
            "temor://history/7f3a0000000000ff",
        ),
        (
            TemorUrl::History(HistoryScope::In("/Users/po/my ml/".to_owned())),
            "temor://history?in=/Users/po/my%20ml/",
        ),
        (TemorUrl::Downloads, "temor://downloads"),
    ];
    for (url, text) in table {
        assert_eq!(url.to_string(), text);
        assert_eq!(text.parse::<TemorUrl>().as_ref(), Ok(&url), "{text}");
    }
}

#[test]
fn what_would_end_a_path_is_encoded() {
    let url = local("/tmp/a b#c?d%e/");
    assert_eq!(url.to_string(), "temor://local/tmp/a%20b%23c%3Fd%25e/");
    assert_eq!(url.to_string().parse::<TemorUrl>(), Ok(url));
    let wide = local("/tmp/日本/");
    assert_eq!(wide.to_string(), "temor://local/tmp/%E6%97%A5%E6%9C%AC/");
    assert_eq!(wide.to_string().parse::<TemorUrl>(), Ok(wide));
}

#[test]
fn what_is_not_a_temor_url_is_refused_with_why() {
    for text in [
        "https://local/tmp/",
        "temor://",
        "temor://local",
        "temor://history/nope",
        "temor://history?q=x",
        "temor://local/tmp/?x=1",
        "temor://local/tmp/#7f3a0000000000ff/twelve",
        "temor://local/tmp/#short",
        "temor://a b/tmp/",
        "temor://local/%ff/",
    ] {
        assert!(text.parse::<TemorUrl>().is_err(), "{text}");
    }
}

#[test]
fn a_host_cannot_take_a_name_the_url_keeps() {
    for name in [
        "local",
        "history",
        "downloads",
        "",
        "a/b",
        "a b",
        "user@box",
        "box:22",
    ] {
        assert!(Host::named(name).is_err(), "{name:?}");
    }
    assert_eq!(Host::named("box-2"), Ok(Host::Named("box-2".to_owned())));
}

#[test]
fn a_host_named_like_a_page_begins_with_it_and_is_still_a_host() {
    let url = TemorUrl::Location(Location {
        host: Host::named("historybox").expect("a name"),
        path: "/".to_owned(),
        within: None,
    });
    assert_eq!(url.to_string().parse::<TemorUrl>(), Ok(url));
}

proptest! {
    #[test]
    fn any_place_reads_back_as_itself(
        host in prop_oneof![Just(None), "[a-z][a-z0-9.-]{0,12}".prop_map(Some)],
        parts in prop::collection::vec("[^/\u{0}]{1,8}", 0..4),
        folder in any::<bool>(),
        session in proptest::option::of(any::<u64>()),
        block in proptest::option::of(any::<u64>()),
    ) {
        let host = match host {
            None => Host::Local,
            Some(name) => match Host::named(&name) {
                Ok(host) => host,
                Err(_) => return Ok(()),
            },
        };
        let mut path = format!("/{}", parts.join("/"));
        if folder && !path.ends_with('/') {
            path.push('/');
        }
        let within = session.map(|bits| Within {
            session: SessionId::from_bits(bits),
            block: block.map(BlockId),
        });
        let url = TemorUrl::Location(Location { host, path, within });
        prop_assert_eq!(url.to_string().parse::<TemorUrl>(), Ok(url));
    }
}

use super::*;

fn host() -> TailnetHost {
    TailnetHost {
        node: TailnetNode::new("nPXS7a2CNTRL"),
        name: "box".to_owned(),
        dns: "box.tail1234.ts.net".to_owned(),
        addresses: vec![
            "100.64.0.7".parse().unwrap(),
            "fd7a:115c::7".parse().unwrap(),
        ],
        reach: Reach::Online,
        last_seen: None,
        ssh: TailscaleSsh::On,
        owner: TailnetOwner::Mine,
    }
}

#[test]
fn an_alias_is_reached_by_dns_name_or_address() {
    let cases = [
        ("box.tail1234.ts.net", true),
        ("BOX.tail1234.ts.net.", true),
        ("100.64.0.7", true),
        ("fd7a:115c::7", true),
        ("100.64.0.8", false),
        ("box", false),
        ("", false),
        ("other.tail1234.ts.net", false),
    ];
    for (host_name, want) in cases {
        assert_eq!(host().is_reached_by(host_name), want, "{host_name:?}");
    }
}

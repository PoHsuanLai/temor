//! Parsing and resolution against in-memory fixtures. The real `~/.ssh` is never read.

use std::path::Path;

use super::model::IssueReason;
use super::support::{aliases, load, one};

#[test]
fn reads_basic_host_settings_with_case_and_equals() {
    let list = load(&[(
        "config",
        "HOST prod\n  HostName=10.0.0.5\n  user = deploy\n  PORT 2222\n  identityfile ~/.ssh/prod_ed\n  ProxyJump bastion\n",
    )]);
    assert!(list.issues.is_empty(), "{:?}", list.issues);
    let prod = one(&list, "prod");
    assert_eq!(prod.hostname.as_str(), "10.0.0.5");
    assert_eq!(
        prod.user.map(|u| u.as_str().to_owned()).as_deref(),
        Some("deploy")
    );
    assert_eq!(prod.port.map(|p| p.get()), Some(2222));
    assert_eq!(prod.identity_files[0].as_written(), "~/.ssh/prod_ed");
    assert_eq!(
        prod.identity_files[0]
            .expanded(Some(Path::new("/fake/home")))
            .as_deref(),
        Some(Path::new("/fake/home/.ssh/prod_ed"))
    );
    assert_eq!(
        prod.proxy_jump.map(|p| p.as_str().to_owned()).as_deref(),
        Some("bastion")
    );
}

#[test]
fn hostname_falls_back_to_the_alias_and_quoted_values_keep_spaces() {
    let list = load(&[(
        "config",
        "Host plain\nHost quoted\n  IdentityFile \"~/.ssh/my key\"\n  IdentityFile \"a\\\"b\"\n",
    )]);
    assert_eq!(one(&list, "plain").hostname.as_str(), "plain");
    let quoted = one(&list, "quoted");
    assert_eq!(quoted.identity_files[0].as_written(), "~/.ssh/my key");
    assert_eq!(quoted.identity_files[1].as_written(), "a\"b");
}

#[test]
fn wildcard_only_patterns_make_no_row_but_supply_defaults() {
    let list = load(&[(
        "config",
        "Host *\n  User fallback\nHost web1\n  User own\nHost web2\n",
    )]);
    assert_eq!(aliases(&list), vec!["web1", "web2"]);
    // The first value wins, so `Host *` (first) beats the later `User own`.
    assert_eq!(
        one(&list, "web1")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("fallback")
    );
    assert_eq!(
        one(&list, "web2")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("fallback")
    );
}

#[test]
fn specific_block_before_wildcard_wins() {
    let list = load(&[(
        "config",
        "Host web1\n  User own\nHost *\n  User fallback\n  Port 2200\n",
    )]);
    let web1 = one(&list, "web1");
    assert_eq!(
        web1.user.map(|u| u.as_str().to_owned()).as_deref(),
        Some("own")
    );
    assert_eq!(web1.port.map(|p| p.get()), Some(2200));
}

#[test]
fn negated_pattern_excludes_the_block() {
    let list = load(&[(
        "config",
        "Host web* !web2\n  User deploy\nHost web1 web2\n  Port 8022\n",
    )]);
    assert_eq!(aliases(&list), vec!["web1", "web2"]);
    assert_eq!(
        one(&list, "web1")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("deploy")
    );
    assert!(one(&list, "web2").user.is_none());
    assert_eq!(one(&list, "web2").port.map(|p| p.get()), Some(8022));
}

#[test]
fn match_blocks_are_skipped_without_failing() {
    let list = load(&[(
        "config",
        "Match host bar exec \"test -f /x\"\n  User ignored\nHost bar\n  User mine\n",
    )]);
    assert_eq!(aliases(&list), vec!["bar"]);
    assert_eq!(
        one(&list, "bar")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("mine")
    );
    assert!(
        list.issues
            .iter()
            .any(|i| i.reason == IssueReason::MatchSkipped)
    );
}

#[test]
fn identity_files_accumulate_across_matching_blocks() {
    let list = load(&[(
        "config",
        "Host *\n  IdentityFile ~/.ssh/a\nHost h\n  IdentityFile ~/.ssh/b\n",
    )]);
    let entry = one(&list, "h");
    let files: Vec<&str> = entry
        .identity_files
        .iter()
        .map(|f| f.as_written())
        .collect();
    assert_eq!(files, vec!["~/.ssh/a", "~/.ssh/b"]);
}

#[test]
fn bad_lines_become_issues_and_the_rest_still_loads() {
    let list = load(&[(
        "config",
        "Host good\n  Port abc\n  Port 0\n  User\n  HostName \"unclosed\n  Port 2022 2023\n  Bogus nothing\n",
    )]);
    let good = one(&list, "good");
    assert!(good.port.is_none());
    let reasons: Vec<&IssueReason> = list.issues.iter().map(|i| &i.reason).collect();
    assert!(
        reasons
            .iter()
            .any(|r| matches!(r, IssueReason::BadValue { keyword, .. } if keyword == "Port"))
    );
    assert!(
        reasons
            .iter()
            .any(|r| matches!(r, IssueReason::MissingValue { keyword } if keyword == "User"))
    );
    assert!(reasons.contains(&&IssueReason::UnterminatedQuote));
    assert_eq!(list.issues.len(), 5, "{:?}", list.issues);
}

#[test]
fn hostile_host_names_are_not_rows() {
    let list = load(&[(
        "config",
        "Host -oProxyCommand=evil\nHost \"a b\"\nHost ok\nHost $(touch x)\n",
    )]);
    assert_eq!(aliases(&list), vec!["ok"]);
    let unsafe_names = list
        .issues
        .iter()
        .filter(|i| matches!(i.reason, IssueReason::UnsafeAlias { .. }))
        .count();
    // `-oProxyCommand=evil`, `a b`, `$(touch` and `x)`.
    assert_eq!(unsafe_names, 4, "{:?}", list.issues);
}

#[test]
fn duplicate_names_collapse_case_insensitively() {
    let list = load(&[("config", "Host Web\n  User a\nHost web\n  Port 22\n")]);
    assert_eq!(aliases(&list), vec!["Web"]);
    assert_eq!(one(&list, "Web").port.map(|p| p.get()), Some(22));
}

#[test]
fn missing_config_is_an_empty_list_not_an_issue() {
    let list = load(&[]);
    assert!(list.entries.is_empty());
    assert!(list.issues.is_empty());
}

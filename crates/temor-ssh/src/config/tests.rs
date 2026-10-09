//! Config parsing against in-memory fixtures. The real `~/.ssh` is never read.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::glob::glob_match;
use super::include::{ConfigFiles, ConfigPaths};
use super::known_host_names;
use super::lex::lex_line;
use super::load_host_list;
use super::model::{HostEntry, HostList, IssueReason};

const SSH_DIR: &str = "/fake/home/.ssh";

/// An in-memory config directory: file path to text.
struct Fixture(BTreeMap<PathBuf, String>);

impl ConfigFiles for Fixture {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        let mut found: Vec<PathBuf> = self
            .0
            .keys()
            .filter(|path| path.parent() == Some(dir))
            .cloned()
            .collect();
        found.sort();
        Ok(found)
    }
}

fn load(files: &[(&str, &str)]) -> HostList {
    let map = files
        .iter()
        .map(|(name, text)| (PathBuf::from(SSH_DIR).join(name), (*text).to_owned()))
        .collect();
    let paths = ConfigPaths {
        config: PathBuf::from(SSH_DIR).join("config"),
        ssh_dir: PathBuf::from(SSH_DIR),
        home: Some(PathBuf::from("/fake/home")),
    };
    load_host_list(&Fixture(map), &paths)
}

fn one(list: &HostList, alias: &str) -> HostEntry {
    list.entries
        .iter()
        .find(|entry| entry.alias.as_str() == alias)
        .cloned()
        .unwrap_or_else(|| panic!("no entry for {alias}: {:?}", list.entries))
}

fn aliases(list: &HostList) -> Vec<&str> {
    list.entries.iter().map(|e| e.alias.as_str()).collect()
}

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
fn includes_resolve_relative_to_ssh_dir_and_globs_in_order() {
    let list = load(&[
        (
            "config",
            "Include conf.d/*.conf\nInclude missing-file\nHost top\n",
        ),
        ("conf.d/b.conf", "Host second\n  User b\n"),
        ("conf.d/a.conf", "Host first\n  User a\n"),
        ("conf.d/ignored.txt", "Host nope\n"),
    ]);
    assert!(list.issues.is_empty(), "{:?}", list.issues);
    assert_eq!(aliases(&list), vec!["first", "second", "top"]);
    assert_eq!(
        one(&list, "second")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("b")
    );
}

#[test]
fn include_cycles_stop_with_an_issue() {
    let list = load(&[("config", "Include config\nHost loop\n")]);
    assert!(
        list.issues
            .iter()
            .any(|i| i.reason == IssueReason::IncludeTooDeep)
    );
    assert_eq!(aliases(&list), vec!["loop"]);
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

#[test]
fn lexer_handles_quotes_escapes_and_comments() {
    let line = lex_line("  Match \"a b\" \"c\\\\d\\\"e\" x\"y z\"")
        .ok()
        .flatten();
    let line = line.expect("a keyword line");
    assert_eq!(line.keyword, "match");
    assert_eq!(line.args, vec!["a b", "c\\d\"e", "xy z"]);
    assert!(lex_line("# comment").is_ok_and(|l| l.is_none()));
    assert!(lex_line("   ").is_ok_and(|l| l.is_none()));
    assert!(lex_line("User \"open").is_err());
}

#[test]
fn glob_matching_is_case_insensitive_with_wildcards() {
    assert!(glob_match("web*", "WEB-1"));
    assert!(glob_match("w?b", "web"));
    assert!(glob_match("*", ""));
    assert!(glob_match("a*b*c", "axxbyyc"));
    assert!(!glob_match("a*b", "ac"));
    assert!(!glob_match("w?b", "wb"));
    assert!(glob_match("*.conf", "x.conf"));
}

#[test]
fn known_hosts_names_skip_hashed_wildcards_and_markers() {
    let text = "\
# comment
example.com,10.0.0.1 ssh-ed25519 AAAA
[alpha]:2222 ssh-ed25519 AAAA
|1|hashedvalue= ssh-rsa AAAA
*.internal ssh-rsa AAAA
!blocked.example ssh-rsa AAAA
@cert-authority *.corp ssh-rsa AAAA
example.com ssh-rsa AAAA
";
    let names: Vec<String> = known_host_names(text)
        .iter()
        .map(|n| n.as_str().to_owned())
        .collect();
    assert_eq!(names, vec!["example.com", "10.0.0.1", "alpha"]);
}

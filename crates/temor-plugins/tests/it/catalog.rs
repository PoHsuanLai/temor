//! Finding manifests and choosing the plugins that serve a request.

use crate::support::{ALL, Host};

fn ids(plugins: Vec<temor_plugins::Plugin>) -> Vec<String> {
    plugins.iter().map(|p| p.id().to_owned()).collect()
}

#[test]
fn manifests_that_temor_cannot_use_are_set_aside_with_a_line_each() {
    let host = Host::new();
    host.install("good", &[], ALL);
    // name of the file, its text, the words the problem line must hold
    let bad: &[(&str, &str, &str)] = &[
        (
            "lens-no-trigger",
            "[[provides]]\ncapability = \"block.lens\"\n",
            "block.lens applies to nothing",
        ),
        (
            "adapter-no-agent",
            "[[provides]]\ncapability = \"agent.adapter\"\n",
            "agent.adapter applies to nothing",
        ),
        (
            "facts-no-tables",
            "[[provides]]\ncapability = \"row.facts\"\n",
            "row.facts applies to nothing",
        ),
        (
            "unknown-key",
            "[[provides]]\ncapability = \"row.facts\"\ntables = [\"folder\"]\ncolour = \"red\"\n",
            "not a manifest",
        ),
        (
            "unknown-capability",
            "[[provides]]\ncapability = \"block.paint\"\n",
            "not a manifest",
        ),
    ];
    for (id, provides, _) in bad {
        host.install(id, &[], provides);
    }
    let catalog = host.catalog();
    assert_eq!(ids(catalog.plugins()), vec!["good"]);
    assert_eq!(
        catalog.problems().len(),
        bad.len(),
        "{:?}",
        catalog.problems()
    );
    for (id, _, words) in bad {
        let line = catalog.problems().iter().find(|l| l.contains(id));
        assert!(
            line.is_some_and(|l| l.contains(words)),
            "{id}: {:?}",
            catalog.problems()
        );
    }
}

#[test]
fn a_request_goes_to_the_plugins_whose_entry_fits_it() {
    let host = Host::new();
    host.install("a", &[], ALL);
    host.install(
        "b",
        &[],
        "[[provides]]\ncapability = \"open.provider\"\n\n[[provides]]\ncapability = \"block.lens\"\ntrigger = \"ls\"\n",
    );
    let catalog = host.catalog();
    let routed: Vec<(String, String)> = catalog
        .providers("gh pr list")
        .into_iter()
        .map(|r| (r.plugin.id().to_owned(), r.text))
        .collect();
    assert_eq!(
        routed,
        vec![
            ("a".to_owned(), "pr list".to_owned()),
            ("b".to_owned(), "gh pr list".to_owned())
        ]
    );
    let table: Vec<(&str, Vec<String>)> = vec![
        (
            "lens: kubectl get pods",
            ids(catalog.lenses("kubectl get pods")),
        ),
        ("lens: ls -l", ids(catalog.lenses("ls -l"))),
        ("lens: kubectl", ids(catalog.lenses("kubectl"))),
        ("facts: folder", ids(catalog.facts_sources("folder"))),
        ("facts: host", ids(catalog.facts_sources("host"))),
    ];
    let want: Vec<(&str, Vec<&str>)> = vec![
        ("lens: kubectl get pods", vec!["a"]),
        ("lens: ls -l", vec!["b"]),
        ("lens: kubectl", vec![]),
        ("facts: folder", vec!["a"]),
        ("facts: host", vec![]),
    ];
    for ((name, got), (_, want)) in table.iter().zip(want) {
        assert_eq!(got, &want, "{name}");
    }
    assert_eq!(
        catalog.adapter("claude").map(|p| p.id().to_owned()),
        Some("a".to_owned())
    );
    assert_eq!(catalog.adapter("codex"), None);
}

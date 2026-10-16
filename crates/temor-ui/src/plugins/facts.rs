//! `row.facts`: what plugins add to a folder's chips and to its sidebar row.
//!
//! A plugin's facts join the built-in ones (branch, changes, versions): they come after them, in
//! the order the plugins serve. A plugin that failed has no facts here, so it shows nothing.

use temor_plugins::message::Fact;

/// One fact a plugin gave about a pane's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFact {
    /// What the person calls the plugin.
    pub plugin: String,
    /// The label (`cluster`).
    pub label: String,
    /// The value (`prod`).
    pub value: String,
}

/// `facts` as one plugin's `PluginFact`s.
pub fn from_plugin(plugin: &str, facts: Vec<Fact>) -> Vec<PluginFact> {
    facts
        .into_iter()
        .map(|fact| PluginFact {
            plugin: plugin.to_owned(),
            label: fact.label,
            value: fact.value,
        })
        .collect()
}

/// The chip's words for a fact: the label in the chip's colour, then the value.
pub fn chip_words(fact: &PluginFact) -> (&str, &str) {
    (&fact.label, &fact.value)
}

/// The facts on one line (`cluster prod · region eu`), none when there are none.
pub fn summary(facts: &[PluginFact]) -> Option<String> {
    let parts: Vec<String> = facts
        .iter()
        .map(|fact| format!("{} {}", fact.label, fact.value))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}

/// The sidebar row's second line: the branch (with its dot) and the plugins' summary, joined.
pub fn with_summary(branch: Option<String>, plugins: Option<String>) -> Option<String> {
    match (branch, plugins) {
        (Some(branch), Some(plugins)) => Some(format!("{branch} \u{b7} {plugins}")),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(label: &str, value: &str) -> PluginFact {
        PluginFact {
            plugin: "p".to_owned(),
            label: label.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn plugin_facts_follow_the_branch_on_the_sidebar_row() {
        let one = vec![fact("cluster", "prod")];
        let two = vec![fact("cluster", "prod"), fact("region", "eu")];
        type Row<'a> = (&'a str, Option<&'a str>, &'a [PluginFact], Option<&'a str>);
        let table: &[Row] = &[
            ("nothing", None, &[], None),
            ("branch alone", Some("main"), &[], Some("main")),
            ("facts alone", None, &one, Some("cluster prod")),
            (
                "both",
                Some("main"),
                &two,
                Some("main \u{b7} cluster prod \u{b7} region eu"),
            ),
        ];
        for (why, branch, facts, want) in table {
            let got = with_summary(branch.map(str::to_owned), summary(facts));
            assert_eq!(got.as_deref(), *want, "{why}");
        }
    }

    #[test]
    fn a_plugins_facts_carry_its_name() {
        let facts = from_plugin(
            "Kube",
            vec![Fact {
                label: "cluster".to_owned(),
                value: "prod".to_owned(),
            }],
        );
        assert_eq!(
            facts,
            vec![PluginFact {
                plugin: "Kube".to_owned(),
                label: "cluster".to_owned(),
                value: "prod".to_owned(),
            }]
        );
        assert_eq!(chip_words(&facts[0]), ("cluster", "prod"));
    }
}

//! `row.facts`: what plugins say about a folder, for its sidebar row.
//!
//! In the order the plugins serve. A plugin that failed has no facts here, so it shows nothing.

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

/// The facts on one line (`cluster prod · region eu`), none when there are none.
pub fn summary(facts: &[PluginFact]) -> Option<String> {
    let parts: Vec<String> = facts
        .iter()
        .map(|fact| format!("{} {}", fact.label, fact.value))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
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
    fn the_summary_joins_the_facts_on_one_line() {
        assert_eq!(summary(&[]), None);
        let two = vec![fact("cluster", "prod"), fact("region", "eu")];
        assert_eq!(
            summary(&two).as_deref(),
            Some("cluster prod \u{b7} region eu")
        );
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
    }
}

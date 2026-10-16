//! The words that route a command or a query to a plugin.

/// Words a plugin answers to: a command's first words (`kubectl get`) for a lens, the first word
/// of a query (`gh`) for a provider. Matching is by whole words, never by characters: `gh`
/// does not route `ghost`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Trigger(Vec<String>);

impl Trigger {
    /// The trigger spelled `text`: its words, separated by any blanks.
    pub fn new(text: &str) -> Trigger {
        Trigger(text.split_whitespace().map(str::to_owned).collect())
    }

    /// Whether it has no words (a provider with none answers every query).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether `command` starts with these words. An empty trigger matches nothing here: a lens
    /// always names the commands it draws.
    pub fn starts(&self, command: &str) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let mut words = command.split_whitespace();
        self.0
            .iter()
            .all(|want| words.next() == Some(want.as_str()))
    }

    /// The query a provider gets for `typed`, or `None` when the query is not for it. An empty
    /// trigger takes every query whole; otherwise the query must start with the trigger's words
    /// and the provider gets the rest, without leading blanks.
    pub fn route(&self, typed: &str) -> Option<String> {
        if self.0.is_empty() {
            return Some(typed.to_owned());
        }
        let mut rest = typed.trim_start();
        for want in &self.0 {
            let word = rest.split_whitespace().next()?;
            if word != want {
                return None;
            }
            rest = rest[word.len()..].trim_start();
        }
        Some(rest.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trigger_matches_whole_words_at_the_start() {
        // trigger, command, starts, routed query
        let table: &[(&str, &str, bool, Option<&str>)] = &[
            ("gh", "gh pr list", true, Some("pr list")),
            ("gh", "  gh   pr", true, Some("pr")),
            ("gh", "gh", true, Some("")),
            ("gh", "ghost", false, None),
            ("gh", "echo gh", false, None),
            ("kubectl get", "kubectl get pods -A", true, Some("pods -A")),
            ("kubectl get", "kubectl", false, None),
            ("kubectl get", "kubectl describe", false, None),
            ("", "anything", false, Some("anything")),
        ];
        for (trigger, text, starts, routed) in table {
            let trigger_ = Trigger::new(trigger);
            assert_eq!(
                trigger_.starts(text),
                *starts,
                "{trigger:?} starts {text:?}"
            );
            assert_eq!(
                trigger_.route(text).as_deref(),
                *routed,
                "{trigger:?} routes {text:?}"
            );
        }
    }
}

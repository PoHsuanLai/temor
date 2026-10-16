//! What a plugin says is shown to a person, so it is bounded and cleaned before it goes anywhere:
//! no control characters, no walls of text, no thousand rows.

use crate::message::{Action, Batch, Fact, Item};

/// The longest a fact's label is shown.
pub const LABEL_CHARS: usize = 24;
/// The longest a fact's value is shown.
pub const VALUE_CHARS: usize = 60;
/// The most facts one plugin adds for a folder.
pub const MOST_FACTS: usize = 6;
/// The longest a row's title or subtitle is shown.
pub const ROW_CHARS: usize = 200;
/// The most rows one plugin gives for a keystroke.
pub const MOST_ROWS: usize = 50;
/// The most secondary actions a row keeps.
pub const MOST_ACTIONS: usize = 8;

/// `text` on one line: every control character (a newline included) becomes a space, runs of
/// blanks become one, and the ends are trimmed. Cut to `most` characters.
pub fn one_line(text: &str, most: usize) -> String {
    let mut out = String::new();
    let mut blank = true;
    for c in text.chars() {
        let c = if c.is_control() || c == '\u{2800}' {
            ' '
        } else {
            c
        };
        if c.is_whitespace() {
            if !blank {
                out.push(' ');
            }
            blank = true;
        } else {
            out.push(c);
            blank = false;
        }
    }
    let kept: String = out.trim_end().chars().take(most).collect();
    kept.trim_end().to_owned()
}

/// The facts a plugin gave, cleaned: a fact without a label or a value is dropped, and no more
/// than [`MOST_FACTS`] are kept.
pub fn tidy_facts(facts: Vec<Fact>) -> Vec<Fact> {
    facts
        .into_iter()
        .map(|fact| Fact {
            label: one_line(&fact.label, LABEL_CHARS),
            value: one_line(&fact.value, VALUE_CHARS),
        })
        .filter(|fact| !fact.label.is_empty() && !fact.value.is_empty())
        .take(MOST_FACTS)
        .collect()
}

/// The batch for keystroke `generation`, cleaned; `None` when it answers an older keystroke. A row
/// with no id or no title is dropped, and the best-scored rows come first.
pub fn tidy_batch(batch: Batch, generation: u64) -> Option<Batch> {
    if batch.generation != generation {
        return None;
    }
    let mut items: Vec<Item> = batch.items.into_iter().filter_map(tidy_item).collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.score));
    items.truncate(MOST_ROWS);
    Some(Batch { generation, items })
}

fn tidy_item(item: Item) -> Option<Item> {
    let title = one_line(&item.title, ROW_CHARS);
    if item.id.is_empty() || title.is_empty() {
        return None;
    }
    let actions = item
        .actions
        .into_iter()
        .map(|action| Action {
            id: action.id,
            label: one_line(&action.label, ROW_CHARS),
        })
        .filter(|action| !action.id.is_empty() && !action.label.is_empty())
        .take(MOST_ACTIONS)
        .collect();
    Some(Item {
        title,
        subtitle: one_line(&item.subtitle, ROW_CHARS),
        actions,
        ..item
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Icon;

    fn fact(label: &str, value: &str) -> Fact {
        Fact {
            label: label.to_owned(),
            value: value.to_owned(),
        }
    }

    fn item(id: &str, title: &str, score: u32) -> Item {
        Item {
            id: id.to_owned(),
            title: title.to_owned(),
            subtitle: String::new(),
            icon: Icon::None,
            score,
            actions: Vec::new(),
        }
    }

    #[test]
    fn text_is_one_clean_line_of_bounded_length() {
        let long = "x".repeat(100);
        let table: &[(&str, &str, usize, &str)] = &[
            ("plain", "prod", 10, "prod"),
            ("newline", "a\nb", 10, "a b"),
            ("escape", "a\u{1b}[31mb", 10, "a [31mb"),
            ("braille blank is a space", "a\u{2800}\u{2800}b", 10, "a b"),
            ("runs of blanks", "  a \t  b  ", 10, "a b"),
            ("cut", &long, 5, "xxxxx"),
            ("only blanks", " \n ", 5, ""),
        ];
        for (why, text, most, want) in table {
            assert_eq!(one_line(text, *most), *want, "{why}");
        }
    }

    #[test]
    fn facts_without_a_label_or_value_are_dropped_and_the_rest_capped() {
        let many: Vec<Fact> = (0..10).map(|n| fact(&format!("k{n}"), "v")).collect();
        assert_eq!(tidy_facts(many).len(), MOST_FACTS);
        let kept = tidy_facts(vec![
            fact("cluster", "prod"),
            fact("", "x"),
            fact("empty", " \n"),
        ]);
        assert_eq!(kept, vec![fact("cluster", "prod")]);
    }

    #[test]
    fn a_batch_keeps_its_best_clean_rows_and_a_stale_one_is_dropped() {
        let batch = |generation| Batch {
            generation,
            items: vec![
                item("a", "low", 10),
                item("", "no id", 99),
                item("b", "  ", 99),
                item("c", "high\nrow", 900),
            ],
        };
        assert_eq!(tidy_batch(batch(3), 4), None);
        let kept: Option<Vec<String>> =
            tidy_batch(batch(4), 4).map(|b| b.items.into_iter().map(|i| i.title).collect());
        assert_eq!(kept, Some(vec!["high row".to_owned(), "low".to_owned()]));
    }
}

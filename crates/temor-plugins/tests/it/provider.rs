//! `open.provider` through a real plugin: streaming by generation, cancelling, choosing a row.

use std::cell::{Cell, RefCell};

use temor_plugins::PluginError;
use temor_plugins::message::{Activation, Batch, Choice, Icon, Item};

use crate::support::{ALL, Host, launcher};

fn titles(batches: &[Batch]) -> Vec<Vec<String>> {
    batches
        .iter()
        .map(|b| b.items.iter().map(|i| i.title.clone()).collect())
        .collect()
}

#[test]
fn a_query_streams_the_rows_of_its_generation_best_first() {
    let host = Host::new();
    host.install("p", &[], ALL);
    let plugin = host.plugin("p");
    // text, generation, the batches that reach the bar
    let table: Vec<(&str, u64, Vec<Vec<&str>>)> = vec![
        ("pr", 7, vec![vec!["pr high", "pr low"]]),
        ("stale", 7, vec![]),
    ];
    for (text, generation, want) in table {
        let got = RefCell::new(Vec::new());
        let done = launcher().query(&plugin, text, generation, &|| false, &mut |batch| {
            got.borrow_mut().push(batch)
        });
        assert_eq!(done, Ok(()), "{text}");
        assert_eq!(titles(&got.borrow()), want, "{text}");
    }
}

#[test]
fn the_next_keystroke_cancels_a_query_that_is_still_streaming() {
    let host = Host::new();
    host.install("p", &[], ALL);
    let plugin = host.plugin("p");
    let seen = Cell::new(0_u32);
    let done = launcher().query(&plugin, "slow", 1, &|| seen.get() >= 2, &mut |_| {
        seen.set(seen.get() + 1)
    });
    assert!(
        matches!(done, Err(PluginError::Cancelled { .. })),
        "{done:?}"
    );
    assert!(seen.get() >= 2);
}

#[test]
fn choosing_a_row_gets_the_activation_the_plugin_names() {
    let host = Host::new();
    host.install("p", &[], ALL);
    let plugin = host.plugin("p");
    let item = |id: &str| Item {
        id: id.to_owned(),
        title: id.to_owned(),
        subtitle: String::new(),
        icon: Icon::None,
        score: 0,
        actions: Vec::new(),
    };
    let copy = launcher().activate(&plugin, &item("42"), &Choice::Primary);
    assert_eq!(
        copy,
        Ok(Activation::Copy {
            text: "42".to_owned()
        })
    );
    let evil = launcher().activate(&plugin, &item("evil"), &Choice::Primary);
    let refused = evil.map(temor_plugins::effect_of);
    assert_eq!(refused, Ok(Err(temor_plugins::Refused::BadUrl)));
}

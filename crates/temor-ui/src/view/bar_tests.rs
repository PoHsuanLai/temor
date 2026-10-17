//! The bar's headers: a plugin's rows sit under its name.

use temor_core::bar::{BarAction, BarRow, RowGroup, RowKey, Section};
use temor_core::vocab::text::PluginId;

use super::{runs, title_of};

fn plugin_row(plugin: &str, item: &str) -> BarRow {
    BarRow {
        key: RowKey(format!("plugin:{plugin}:{item}")),
        group: RowGroup::Plugin,
        title: item.to_owned(),
        action: BarAction::Plugin {
            plugin: PluginId(plugin.to_owned()),
            item: item.to_owned(),
        },
    }
}

#[test]
fn each_plugin_has_a_header_with_its_name_and_the_rest_keep_their_words() {
    let rows = [
        BarRow {
            key: RowKey("new-tab".to_owned()),
            group: RowGroup::Run,
            title: "New tab".to_owned(),
            action: BarAction::NewTab,
        },
        plugin_row("gh", "pr 1"),
        plugin_row("gh", "pr 2"),
        plugin_row("kube", "prod"),
    ];
    let name = |id: &PluginId| match id.as_str() {
        "gh" => "GitHub".to_owned(),
        other => other.to_owned(),
    };
    let headers: Vec<(String, usize)> = runs(&rows)
        .into_iter()
        .map(|(section, run)| (title_of(&section, name), run.len()))
        .collect();
    assert_eq!(
        headers,
        [
            ("Actions".to_owned(), 1),
            ("GitHub".to_owned(), 2),
            ("kube".to_owned(), 1)
        ]
    );
    assert_eq!(title_of(&Section::Ask, name), "Ask");
}

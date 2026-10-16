//! `open.provider`: the rows plugins offered for the text in the bar.

use temor_core::bar::PluginRow;
use temor_core::vocab::text::PluginId;
use temor_plugins::Plugin;
use temor_plugins::message::Item;

/// A row, and the plugin it came from, kept until the bar closes so the choice can be handed back
/// with the whole item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The plugin.
    pub plugin: Plugin,
    /// The plugin's row.
    pub item: Item,
}

/// The rows offered for the query now typed.
#[derive(Debug, Clone, Default)]
pub struct OfferBook {
    rows: Vec<Offered>,
}

impl OfferBook {
    /// Forgets every row: a new keystroke starts over.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Adds a batch. A row whose id the plugin gave before takes the place of the earlier one.
    pub fn add(&mut self, plugin: &Plugin, items: Vec<Item>) {
        for item in items {
            let same = |offered: &Offered| {
                offered.plugin.id() == plugin.id() && offered.item.id == item.id
            };
            match self.rows.iter_mut().find(|offered| same(offered)) {
                Some(old) => old.item = item,
                None => self.rows.push(Offered {
                    plugin: plugin.clone(),
                    item,
                }),
            }
        }
    }

    /// The rows for the bar, best score first; equal scores keep the order they arrived in. The
    /// title carries the plugin's name, since the bar lists plugin rows together.
    pub fn plugin_rows(&self) -> Vec<PluginRow> {
        let mut ordered: Vec<&Offered> = self.rows.iter().collect();
        ordered.sort_by_key(|row| std::cmp::Reverse(row.item.score));
        ordered
            .into_iter()
            .map(|offered| PluginRow {
                plugin: PluginId(offered.plugin.id().to_owned()),
                item: offered.item.id.clone(),
                title: format!("{} \u{b7} {}", offered.plugin.name(), offered.item.title),
            })
            .collect()
    }

    /// The row `item` of the plugin `plugin`, if it is still on offer.
    pub fn find(&self, plugin: &str, item: &str) -> Option<&Offered> {
        self.rows
            .iter()
            .find(|offered| offered.plugin.id() == plugin && offered.item.id == item)
    }
}

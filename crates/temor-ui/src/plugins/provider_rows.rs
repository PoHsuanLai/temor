//! The bar takes the rows plugins offered (PLAN §7, `open.provider`). Mounted from
//! `bar/adapter.rs` as a child module, so it reaches the model's private fields.

use ds::prelude::PaletteIn;
use temor_core::bar::{BarContext, BarIn, BarOut, PluginRow, bar_step};

use super::{BarModel, rows_of, walk};

impl BarModel {
    /// Plugins answered for `query` with `rows`, all of them so far (a later answer replaces an
    /// earlier one). An answer for text that is no longer in the field changes nothing. The
    /// palette keeps its highlight where the rows allow.
    pub fn provider_rows(
        self,
        query: String,
        rows: Vec<PluginRow>,
        cx: BarContext<'_>,
    ) -> BarModel {
        let (bar, outs) = bar_step(self.bar, BarIn::ProviderRows { query, rows }, cx);
        let ranked = rows_of(&bar);
        let palette = match outs.contains(&BarOut::Refreshed) {
            true => walk(self.palette, PaletteIn::Refreshed, &ranked).0,
            false => self.palette,
        };
        BarModel { bar, palette }
    }
}

#[cfg(test)]
mod tests {
    use temor_core::bar::{BarAction, BarContext, PathResolver, PluginRow};
    use temor_core::vocab::ai::AiSwitch;
    use temor_core::vocab::text::PluginId;

    use super::BarModel;

    struct Nothing;
    impl PathResolver for Nothing {
        fn resolves(&self, _: &str) -> bool {
            false
        }
    }

    fn row(item: &str) -> PluginRow {
        PluginRow {
            plugin: PluginId("gh".to_owned()),
            name: "GitHub".to_owned(),
            item: item.to_owned(),
            title: item.to_owned(),
        }
    }

    #[test]
    fn plugin_rows_show_for_the_text_they_answered_and_for_nothing_else() {
        let cx = BarContext {
            candidates: &[],
            resolver: &Nothing,
            ai: AiSwitch::Off,
        };
        let model = BarModel::default()
            .toggled(cx)
            .typed("gh pr".to_owned(), cx);
        let stale = model
            .clone()
            .provider_rows("gh p".to_owned(), vec![row("1")], cx);
        assert!(
            stale.rows().is_empty(),
            "an answer to older text is dropped"
        );

        let model = model.provider_rows("gh pr".to_owned(), vec![row("1")], cx);
        let model = model.provider_rows("gh pr".to_owned(), vec![row("1"), row("2")], cx);
        let keys: Vec<&str> = model.rows().iter().map(|r| r.key.0.as_str()).collect();
        assert_eq!(keys, ["plugin:gh:1", "plugin:gh:2"]);
        assert_eq!(
            model.rows()[0].action,
            BarAction::Plugin {
                plugin: PluginId("gh".to_owned()),
                item: "1".to_owned()
            }
        );
    }
}

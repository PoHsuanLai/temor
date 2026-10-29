//! Following `settings.toml` while the window lives (PLAN §14): the font and the cursor apply to
//! every pane that is open, and so does the notice threshold; the scrollback, the shell and the
//! shell offer's lists apply to the panes that start next. (`keys.conf` is followed with the keymap, in
//! `hub::keys`.)

use std::sync::Arc;

use dioxus::prelude::spawn;
use ds::base::spawner::Spawner;
use ds_settings::SettingsDoc;
use temor_client::model::offer::OfferList;

use super::desk::Desk;
use super::model::Hub;
use crate::settings::{TerminalDoc, live_changes};
use crate::view::launch::load_fonts;

impl Hub {
    /// Puts the settings `doc` holds in force: the open panes take the look at once, and the
    /// rest is read by the panes that come. Returns what the person should be told.
    pub(super) fn apply_settings(&mut self, doc: &TerminalDoc) -> Vec<String> {
        let (new, problems) = doc.terminal(self.launch.settings.keys.clone());
        let mut said: Vec<String> = problems
            .into_iter()
            .map(|problem| format!("Settings: {problem}"))
            .collect();
        let change = live_changes(&self.launch.settings, &new);
        for slot in self.slots.values() {
            let mut slot = slot.borrow_mut();
            let Some(session) = slot.session.as_mut() else {
                continue;
            };
            if let Some((font, size, line)) = &change.font {
                let applied =
                    load_fonts(font, *size, *line).and_then(|fonts| session.set_fonts(fonts));
                if let Err(err) = applied {
                    said.push(format!("Settings: the font was not changed: {err}"));
                }
            }
            if let Some(style) = change.cursor
                && let Err(err) = session.set_cursor_style(style)
            {
                said.push(format!("Settings: the cursor was not changed: {err}"));
            }
            if let Some(blink) = change.blink {
                session.set_cursor_blink(blink);
            }
            if let Some(after) = change.notify {
                session.set_notify_after(after);
            }
        }
        self.offer.list = OfferList::new(&new.offer_add, &new.offer_remove);
        self.launch.settings = new;
        said.dedup();
        said
    }
}

/// Reads `settings.toml` again whenever it changes, for as long as the window lives, and puts
/// what it says in force. The watch runs as a task on `spawner`.
pub fn spawn_settings_loop(desk: Desk, spawner: Arc<dyn Spawner>) {
    let store = desk.hub.0.borrow().launch.ambient.settings_store();
    spawn(async move {
        let mut watch = store.watch::<TerminalDoc>(&*spawner);
        while let Some(loaded) = watch.changed().await {
            let mut said = desk.hub.0.borrow_mut().apply_settings(&loaded.value);
            said.extend(
                loaded
                    .diagnostics(TerminalDoc::FILE)
                    .into_iter()
                    .map(|line| format!("Settings: {line}")),
            );
            if let Some(first) = said.into_iter().next() {
                desk.say(first);
            }
            desk.bump();
        }
    });
}

impl super::model::HubRef {
    /// The font the terminals are drawn in, once one is: what the rest of the window sets code in
    /// (a page's text, a peek), so the window has one monospace.
    pub fn grid_font(&self) -> Option<crate::paint::GridFont> {
        let hub = self.0.borrow();
        hub.slots.values().find_map(|slot| {
            let slot = slot.borrow();
            slot.session
                .as_ref()
                .map(|session| session.grid_font().clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use chordkit::Overrides;
    use temor_panes::pane::settings::{FontPoints, ShellChoice};

    use super::{Hub, TerminalDoc};
    use crate::settings::FontSize;

    #[test]
    fn a_changed_file_reaches_the_next_panes_and_the_offer_and_keeps_the_keys() {
        let mut hub = Hub::new(super::super::tests::launch());
        let (keys, _) = Overrides::parse("temor.split-right = Primary+Alt+N\n");
        hub.launch.settings.keys = keys.clone();
        let mut doc = TerminalDoc::default();
        doc.font.size = FontSize(FontPoints::new(20.0).expect("in range"));
        doc.shell.program = "/bin/zsh".to_owned();
        doc.offer.add = vec!["kubectl debug -it".to_owned()];
        // A family that is only blanks cannot be used: one problem, said once.
        doc.font.family = " ".to_owned();
        let said = hub.apply_settings(&doc);
        let settings = &hub.launch.settings;
        assert_eq!(settings.font_size.points(), 20.0);
        assert_eq!(settings.shell, ShellChoice::Program("/bin/zsh".to_owned()));
        assert_eq!(settings.keys, keys);
        assert!(hub.offer.list.matches("kubectl debug -it pod").is_some());
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("font.family"), "{said:?}");
    }
}

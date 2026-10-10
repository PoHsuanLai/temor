//! `context()`: what the person is looking at, as the agent layer wants it. The focused pane's
//! folder and last command are not in the snapshot (its fields have no place for them); an agent
//! reads them with the `temor.pane.cwd` and `temor.command.read` actions, which taint its
//! session. What is here: the window title and the selection (both somebody's words, so
//! untrusted and kept inside the Space) and how many panes are open.

use crate::{APP_BUS_NAME, WindowContext};
use docket_client::ContextSource;
use docket_core::{
    ContextScope, ContextSnapshot, Here, Selection, TextTarget, ViewName, Visible, WindowPrivacy,
};
use porter_core::{AppName, Count, DataClass, LocalSpace, SpaceId};
use prov::{Label, Labelled, Source};

/// Temor's snapshot of `context` for `app`.
pub(super) fn snapshot_of(app: &AppName, context: &WindowContext) -> ContextSnapshot {
    let local = LocalSpace(context.space.local);
    // A link that is not a Space id falls back to the app's own Space: private is the safe side.
    let space = SpaceId::for_app(app, local, context.space.link.as_deref())
        .unwrap_or_else(|_| SpaceId::app(app, local));
    let words = |text: &str| Labelled {
        value: text.to_owned(),
        label: Label::untrusted(
            Source::Screen { app: app.clone() },
            DataClass::Screen,
            space.clone(),
        ),
    };
    let here = match (&context.focused, ViewName::parse("pane")) {
        (Some(_), Ok(view)) => Here::View { view, query: None },
        _ => Here::Nowhere,
    };
    let selection = match &context.selection {
        Some(text) if !text.is_empty() => Selection::Text(words(text)),
        _ => Selection::Nothing,
    };
    ContextSnapshot {
        app: app.clone(),
        window: words(&context.title),
        here,
        selection,
        visible: Visible {
            kind: None,
            items: Vec::new(),
            total: Count(context.pane_count),
        },
        text_target: TextTarget::None,
        privacy: WindowPrivacy::Normal,
    }
}

/// The window as the router's `Context` call reads it.
pub(super) struct WindowSource<W> {
    pub(super) app: AppName,
    pub(super) window: std::sync::Arc<W>,
}

impl<W: crate::WindowPort> ContextSource for WindowSource<W> {
    fn snapshot(&self, _scope: ContextScope) -> ContextSnapshot {
        snapshot_of(&self.app, &self.window.context())
    }
}

/// The app name temor serves under.
pub(super) fn app_name() -> Option<AppName> {
    AppName::parse(APP_BUS_NAME).ok()
}

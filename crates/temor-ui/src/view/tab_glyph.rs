//! A terminal's glyph in the sidebar: the bare prompt, `>_`, drawn in outline as Warp draws its
//! tabs, not quire's solid boxed terminal, which sits too heavy in a list of tabs.

use dioxus::prelude::*;
use ds::base::vocab::CompanionPresence;
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::prelude::{ExternalIcon, IconSize, IconSource, IconView};
use ds::style::icon::url::IconUrl;
use temor_core::presence::{AgentState, companion_presence};

// Lucide's `terminal` (lucide.dev, ISC licence, Copyright (c) Lucide Contributors; see NOTICE),
// as a mask painted in the row's text colour.
const PROMPT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m4 17 6-6-6-6"/><path d="M12 19h8"/></svg>"##;

/// The prompt glyph at `size`.
pub fn prompt_glyph(size: IconSize) -> IconSource {
    IconSource::Symbolic(ExternalIcon {
        url: IconUrl::svg(PROMPT_SVG),
        size,
    })
}

/// What stands for a tab or pane that shows no label of its own: the prompt glyph at `size`, or
/// the companion's orb while an agent in it works or waits.
pub fn presence_face(agent: AgentState, size: IconSize) -> Element {
    match companion_presence(agent) {
        CompanionPresence::Idle => rsx! {
            IconView { source: prompt_glyph(size), size }
        },
        presence => rsx! {
            CompanionOrb { presence, size: OrbSize::Inline }
        },
    }
}

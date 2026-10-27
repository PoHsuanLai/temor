//! A terminal's glyph in the sidebar: the bare prompt, `>_`, drawn in outline as Warp draws its
//! tabs, not quire's solid boxed terminal, which sits too heavy in a list of tabs.

use dioxus::prelude::*;
use ds::base::vocab::CompanionPresence;
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::components::content::avatar::{AvatarFace, AvatarShape, AvatarSize, AvatarTone, PersonHue};
use ds::prelude::{ExternalIcon, IconSize, IconSource, IconView, RowLeading};
use ds::style::icon::url::IconUrl;
use temor_core::presence::{AgentState, companion_presence};

use crate::hub::RowIcon;

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

/// What leads a tab's row: its app's icon as it is, a brand's glyph in white on the brand's
/// colour, or the letter tile, each a 16 px square as a sidebar's favicon is.
pub fn row_leading(icon: &RowIcon) -> RowLeading {
    match icon {
        RowIcon::App(png) => {
            let src = IconUrl::png(png).as_str().to_owned();
            RowLeading::Element(rsx! {
                img { class: "temor-tab-app", src }
            })
        }
        RowIcon::Brand { svg, colour } => {
            let source = IconSource::Symbolic(ExternalIcon {
                url: IconUrl::svg(svg),
                size: IconSize::Micro,
            });
            RowLeading::Element(rsx! {
                span {
                    class: "temor-tab-brand",
                    style: format!("background: #{colour:06x}"),
                    IconView { source, size: IconSize::Micro }
                }
            })
        }
        RowIcon::Letter { initial, key } => RowLeading::Avatar(AvatarFace {
            initial: *initial,
            size: AvatarSize::Size16,
            tone: AvatarTone::Person(PersonHue::of(key)),
            shape: AvatarShape::Square,
        }),
    }
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

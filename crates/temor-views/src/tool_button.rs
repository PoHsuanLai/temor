//! An icon-only button in a toolbar: the block tools, the find bar's steps and the Downloads
//! button are all this. The label names it for assistive technology and `tip` is the hover hint
//! (a caller with a chord for the action writes it with `HubRef::tooltip`).

use dioxus::prelude::*;
use ds::components::content::icon_source::IconSource;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::components::controls::press::Propagation;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;

/// One icon button of a toolbar. `propagation` is `Propagation::Stop` where the toolbar sits over
/// something that must not hear the press.
#[component]
#[allow(non_snake_case)]
pub fn ToolButton(
    icon: Icon,
    #[props(into)] label: String,
    #[props(into)] tip: String,
    #[props(default)] size: ControlSize,
    #[props(default)] availability: Availability,
    #[props(default)] propagation: Propagation,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        Button {
            bezel: Bezel::Toolbar,
            image: ImagePosition::Only,
            size,
            icon: Some(IconSource::Glyph(icon)),
            label,
            title: Some(tip),
            availability,
            propagation,
            onclick: move |_| onclick.call(()),
        }
    }
}

//! The shell offer's bar (PLAN P17): one quiet question under a pane whose typed `ssh`, `docker
//! exec` or the like has started a shell of its own. Nothing is typed into the pane before Yes.

use dioxus::prelude::*;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::model::offer::Reply;
use temor_client::model::vocab::ids::PaneId;

use crate::hub::Desk;

/// The question for `pane`, while there is one.
#[component]
#[allow(non_snake_case)]
pub fn OfferBar(pane: PaneId, desk: Desk) -> Element {
    let _ = (desk.ui)();
    let Some(program) = desk.hub.offer_asking(pane) else {
        return rsx! {};
    };
    let (yes, later, never) = (desk.clone(), desk.clone(), desk);
    rsx! {
        InlineBanner {
            severity: Severity::Info,
            text: "Show commands as blocks in this shell?".to_owned(),
            detail: Some(TextLine::from(format!(
                "You started {program}. temor can type a few lines to set this up here"
            ))),
            actions: rsx! {
                Button {
                    label: "Yes".to_owned(),
                    size: ControlSize::Small,
                    title: Some("Type the lines that set this up".to_owned()),
                    onclick: move |_| yes.offer_answer(pane, Reply::Yes),
                }
                Button {
                    label: "Not now".to_owned(),
                    size: ControlSize::Small,
                    title: Some("Leave this shell as it is".to_owned()),
                    onclick: move |_| later.offer_answer(pane, Reply::NotNow),
                }
                Button {
                    label: format!("Never for {program}"),
                    size: ControlSize::Small,
                    title: Some("Don't ask again for this command".to_owned()),
                    onclick: move |_| never.offer_answer(pane, Reply::Never),
                }
            },
        }
    }
}

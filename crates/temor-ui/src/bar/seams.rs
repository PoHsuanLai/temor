//! What the bar reads from parts of the app that live elsewhere. Each has a default that offers
//! nothing, so the bar works (and says nothing false) until the part exists. The SSH hosts are
//! not a seam: the hub lists them itself (`hub/hosts.rs`).

use std::rc::Rc;

use ds::style::space::list::SpaceId;
use temor_core::bar::{BarAction, PathResolver, SpaceEntry, UnixSecs};
use temor_core::vocab::ai::AiSwitch;

use super::resolver::PathLookup;
use crate::edge::unix_now;
use crate::view::launch::Launch;

/// The Spaces the bar lists and switches to. The Spaces lane implements it.
pub trait SpaceRows {
    /// The Spaces, in the order the switcher lists them. Empty when there are none.
    fn spaces(&self) -> Vec<SpaceEntry>;

    /// Switches to a Space the bar listed. The words come back as the notice when it cannot.
    fn go_to(&self, space: SpaceId) -> Result<(), String>;
}

/// The model behind the bar's AI rows (PLAN §8.3, D6, D11). The rows exist only when this says
/// a model can be asked; there is no stand-in. Picking a row is the person's request.
pub trait ModelRows {
    /// Whether the AI rows may show.
    fn available(&self) -> AiSwitch;

    /// Carries out a picked AI row. The words come back as the notice when it cannot.
    fn ask(&self, row: &BarAction) -> Result<(), String>;
}

/// No Spaces yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSpaces;

impl SpaceRows for NoSpaces {
    fn spaces(&self) -> Vec<SpaceEntry> {
        Vec::new()
    }

    fn go_to(&self, _: SpaceId) -> Result<(), String> {
        Err("Spaces are not available yet.".to_owned())
    }
}

/// No model: the AI rows are absent.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoModel;

impl ModelRows for NoModel {
    fn available(&self) -> AiSwitch {
        AiSwitch::Off
    }

    fn ask(&self, _: &BarAction) -> Result<(), String> {
        Err("No assistant is set up.".to_owned())
    }
}

/// Everything the bar reads from outside, held by the hub.
#[derive(Clone)]
pub struct BarSeams {
    pub spaces: Rc<dyn SpaceRows>,
    pub model: Rc<dyn ModelRows>,
    /// The `PATH` lookup that decides whether the Run row exists.
    pub paths: Rc<dyn PathResolver>,
    /// The wall clock, for the folders' frecency.
    pub now: fn() -> UnixSecs,
}

impl BarSeams {
    /// The defaults for a window: no Spaces or model, and the lookup over the `PATH` the
    /// new shells start with.
    pub fn new(launch: &Launch) -> BarSeams {
        let path = launch.ambient.env.get("PATH").unwrap_or_default();
        BarSeams {
            spaces: Rc::new(NoSpaces),
            model: Rc::new(NoModel),
            paths: Rc::new(PathLookup::new(path.to_owned())),
            now: unix_now,
        }
    }
}

impl std::fmt::Debug for BarSeams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarSeams").finish_non_exhaustive()
    }
}

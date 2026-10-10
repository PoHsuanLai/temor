//! The kit's Spaces handle as temor types it.

use ds::components::app::spaces::SpacesHandle;
use temor_core::space::{Place, Project};

/// The window's Spaces: temor's project in each, and the tabs it was left with as its place.
pub type SpacesOf = SpacesHandle<Project, Place>;

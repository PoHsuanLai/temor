//! A Space is a project (D2, D10): the kit's `Spaces<Project, Place>`, with temor's payload and
//! place.

mod place;
mod project;

pub use place::{Folder, Pin, Place, Tab};
pub use project::{Profile, Project, Route};

/// temor's Spaces: each keeps a [`Project`] and recalls a [`Place`].
pub type TemorSpaces = ds_style::space::list::Spaces<Project, Place>;

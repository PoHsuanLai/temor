//! The accessible name of a control, for the controls that need nothing else in `Common`.

use ds::prelude::Common;

/// `Common` carrying only an accessible name: what assistive technology and tests call the
/// control, such as a button named for the row it acts on.
pub fn named(name: impl Into<String>) -> Common {
    Common {
        aria_label: Some(name.into()),
        ..Common::default()
    }
}

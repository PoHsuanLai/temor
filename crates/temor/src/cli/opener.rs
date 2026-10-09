//! Opening a file with the system's own program. The edge is a trait so the decision above it can
//! be tested; the real one is in `platform`.

use std::path::Path;

use super::error::CliError;

/// Opens files with whatever the computer uses for that.
pub trait Opener {
    fn open(&self, path: &Path) -> Result<(), CliError>;
}

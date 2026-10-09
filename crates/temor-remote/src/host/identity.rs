//! Who this helper is: the hash of its own binary (PLAN §9b: the version is the content hash).

use std::path::Path;

use temor_wire::hello::HelperVersion;

use crate::error::Error;
use crate::files::sha256;

/// The SHA-256 of the executable at `path`.
pub fn hash_executable(path: &Path) -> Result<HelperVersion, Error> {
    let bytes = std::fs::read(path)
        .map_err(|error| Error::io(format!("reading {}", path.display()), error))?;
    Ok(HelperVersion(sha256(&bytes)))
}

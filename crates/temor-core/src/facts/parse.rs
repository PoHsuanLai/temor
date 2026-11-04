//! Reading what the environment says. Anything unreadable is `None`.

use super::model::EnvName;

/// The virtualenv's name: the last part of `VIRTUAL_ENV`. When that folder is just `.venv`, the
/// project folder above it names the environment, as Python's prompt does.
pub fn venv_name(path: &str) -> Option<EnvName> {
    let mut parts = path.rsplit('/').filter(|p| !p.is_empty());
    let last = parts.next()?;
    let name = match last {
        ".venv" | "venv" => parts.next().unwrap_or(last),
        other => other,
    };
    Some(EnvName(name.to_owned()))
}

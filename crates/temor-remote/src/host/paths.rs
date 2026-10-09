//! Where the daemon's socket is.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use temor_wire::hello::HelperVersion;

/// The private folder for the socket: `$XDG_RUNTIME_DIR/temor-remote`, or where there is no
/// runtime directory `~/.cache/temor-remote/run`. `None` when neither can be told.
pub fn runtime_dir(xdg_runtime_dir: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let usable = |value: &OsStr| (!value.is_empty()).then(|| PathBuf::from(value));
    if let Some(runtime) = xdg_runtime_dir.and_then(usable) {
        return Some(runtime.join("temor-remote"));
    }
    home.and_then(usable)
        .map(|home| home.join(".cache").join("temor-remote").join("run"))
}

/// The socket for one helper version: `<dir>/<first 16 hex digits of its hash>.sock`. A new
/// version listens elsewhere, so an update never talks to an old daemon by mistake.
pub fn socket_path(dir: &Path, helper: &HelperVersion) -> PathBuf {
    let hex = helper.0.to_hex();
    dir.join(format!("{}.sock", &hex[..16]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::digest::ContentHash;

    #[test]
    fn the_runtime_dir_wins_then_the_cache_then_nothing() {
        let os = |text: &'static str| Some(OsStr::new(text));
        let cases = [
            (
                os("/run/user/1000"),
                os("/home/me"),
                Some("/run/user/1000/temor-remote"),
            ),
            (
                os(""),
                os("/home/me"),
                Some("/home/me/.cache/temor-remote/run"),
            ),
            (
                None,
                os("/home/me"),
                Some("/home/me/.cache/temor-remote/run"),
            ),
            (None, os(""), None),
            (None, None, None),
        ];
        for (xdg, home, want) in cases {
            assert_eq!(
                runtime_dir(xdg, home),
                want.map(PathBuf::from),
                "{xdg:?} {home:?}"
            );
        }
    }

    #[test]
    fn the_socket_name_follows_the_hash() {
        let helper = HelperVersion(ContentHash([0xab; 32]));
        assert_eq!(
            socket_path(Path::new("/run/x"), &helper),
            PathBuf::from("/run/x/abababababababab.sock")
        );
    }
}

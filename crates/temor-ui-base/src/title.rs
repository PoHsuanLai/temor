//! The window's title (PLAN §7): the pane's own title when the program set one, else the
//! directory it is in, else the app's name.

/// The name the window has when the pane has neither a title nor a directory.
pub const APP_NAME: &str = "temor";

/// The window title for a pane whose program set `pane_title` (empty when it set none) and whose
/// shell reported `cwd`.
pub fn window_title(pane_title: &str, cwd: Option<&str>) -> String {
    if !pane_title.is_empty() {
        return pane_title.to_owned();
    }
    match cwd {
        Some(dir) if !dir.is_empty() => dir.to_owned(),
        _ => APP_NAME.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_NAME, window_title};

    #[test]
    fn the_pane_title_then_the_directory_then_the_app_name() {
        let cases = [
            ("vim notes.md", Some("/home/a"), "vim notes.md"),
            ("", Some("/home/a/src"), "/home/a/src"),
            ("", Some(""), APP_NAME),
            ("", None, APP_NAME),
        ];
        for (pane, cwd, want) in cases {
            assert_eq!(window_title(pane, cwd), want, "{pane:?} {cwd:?}");
        }
    }
}

//! What a tab is called in the sidebar (PLAN §7). The name is the program
//! that runs in the pane while one runs, else the last part of the pane's folder. A window title
//! of the form `user@host:path` is the shell's own and is never used. Pure: the hub reads the
//! inputs and the row draws the result.

use std::path::Path;

/// What names a tab.
#[derive(Debug, Clone, Copy)]
pub struct NameIn<'a> {
    /// The command that is running now, as typed.
    pub running: Option<&'a str>,
    /// The title the program set, empty when it set none.
    pub title: &'a str,
    /// The folder the shell reported.
    pub cwd: Option<&'a str>,
    /// The home folder, written `~`.
    pub home: &'a Path,
    /// What the tab was called when it was made.
    pub fallback: &'a str,
    /// Whether the shell marks its commands (it reported at least one prompt), so that a
    /// command that is not running is known to be idle. Without marks the program's own title
    /// is all there is to go on.
    pub marked: bool,
}

/// The tab's name.
pub fn tab_title(input: NameIn<'_>) -> String {
    let program_runs = input.running.is_some() || !input.marked;
    if program_runs && usable(input.title) {
        return input.title.trim().to_owned();
    }
    if let Some(command) = input.running.map(first_line).filter(|c| !c.is_empty()) {
        return command.to_owned();
    }
    input
        .cwd
        .and_then(|cwd| folder_name(cwd, input.home))
        .unwrap_or_else(|| input.fallback.to_owned())
}

/// A title that names a program: not empty and not the shell's `user@host:path`.
fn usable(title: &str) -> bool {
    !title.trim().is_empty() && !is_host_form(title.trim())
}

/// Whether `title` reads `user@host:path` or `user@host: path`.
pub(super) fn is_host_form(title: &str) -> bool {
    let Some((user, rest)) = title.split_once('@') else {
        return false;
    };
    let Some((host, _path)) = rest.split_once(':') else {
        return false;
    };
    let word = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
    word(user) && word(host)
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

/// The last part of a folder; `~` for the home folder and `/` for the root.
fn folder_name(cwd: &str, home: &Path) -> Option<String> {
    let trimmed = cwd.trim_end_matches('/');
    if trimmed.is_empty() {
        return match cwd.is_empty() {
            true => None,
            false => Some("/".to_owned()),
        };
    }
    if !home.as_os_str().is_empty() && Path::new(trimmed) == home {
        return Some("~".to_owned());
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{NameIn, is_host_form, tab_title};

    fn name(running: Option<&str>, title: &str, cwd: Option<&str>, marked: bool) -> String {
        tab_title(NameIn {
            running,
            title,
            cwd,
            home: Path::new("/home/me"),
            fallback: "Shell",
            marked,
        })
    }

    /// (case, running program, window title, folder, marking, expected name)
    type Case<'a> = (
        &'a str,
        Option<&'a str>,
        &'a str,
        Option<&'a str>,
        bool,
        &'a str,
    );

    #[test]
    fn a_tab_is_named_by_its_program_while_one_runs_else_by_its_folder() {
        const MARKED: bool = true;
        const UNMARKED: bool = false;
        #[rustfmt::skip]
        let rows: &[Case] = &[
            ("program title wins",       Some("vim a.md"),    "vim a.md",        Some("/home/me/doc"),  MARKED,   "vim a.md"),
            ("host-form title ignored",  Some("ssh box"),     "me@box: ~/doc",   Some("/home/me/doc"),  MARKED,   "ssh box"),
            ("idle shell: the folder",   None,                "vim a.md",        Some("/home/me/temor"),MARKED,   "temor"),
            ("idle, host-form title",    None,                "me@box:~/temor",  Some("/home/me/temor"),MARKED,   "temor"),
            ("home is a tilde",          None,                "",                Some("/home/me"),      MARKED,   "~"),
            ("home with a slash",        None,                "",                Some("/home/me/"),     MARKED,   "~"),
            ("root",                     None,                "",                Some("/"),             MARKED,   "/"),
            ("no folder: the made name", None,                "",                None,                  MARKED,   "Shell"),
            ("no marks: a program title",None,                "htop",            Some("/home/me/doc"),  UNMARKED, "htop"),
            ("no marks, host-form",      None,                "me@box:~",        Some("/home/me/doc"),  UNMARKED, "doc"),
            ("running, empty title",     Some("sleep 30"),    "",                Some("/home/me/doc"),  MARKED,   "sleep 30"),
        ];
        for (why, running, title, cwd, marking, want) in rows {
            assert_eq!(name(*running, title, *cwd, *marking), *want, "{why}");
        }
    }

    #[test]
    fn host_forms() {
        for (title, want) in [
            ("me@box:~/doc", true),
            ("me@box: ~/doc", true),
            ("vim me@box.txt", false),
            ("me@box", false),
            ("ssh me@box", false),
        ] {
            assert_eq!(is_host_form(title), want, "{title}");
        }
    }
}

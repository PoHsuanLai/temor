//! Carrying out the row a person chose from a plugin. A plugin may name only what the bar itself
//! does with a row: copy, open, nothing (PLAN D14). `temor_plugins::effect_of` has checked what it
//! named; this hands it to the desktop.

use temor_plugins::Effect;

/// What the desktop does for a chosen row. The words of an `Err` are for the person.
pub trait Hands: std::fmt::Debug {
    /// Puts text on the clipboard.
    fn copy(&self, text: &str) -> Result<(), String>;
    /// Opens a web address.
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// Opens a file or folder.
    fn open_file(&self, path: &str) -> Result<(), String>;
}

/// Does `effect`; the notice to show the person when it could not be done.
pub fn carry_out(effect: &Effect, hands: &dyn Hands) -> Option<String> {
    let done = match effect {
        Effect::Nothing => Ok(()),
        Effect::Copy(text) => hands.copy(text),
        Effect::OpenUrl(url) => hands.open_url(url),
        Effect::OpenFile(path) => hands.open_file(path),
    };
    done.err()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Records what it was asked and fails what `fail` names.
    #[derive(Debug)]
    struct Spy {
        calls: RefCell<Vec<String>>,
        fail: &'static str,
    }

    impl Spy {
        fn note(&self, what: &str, arg: &str) -> Result<(), String> {
            self.calls.borrow_mut().push(format!("{what} {arg}"));
            match self.fail == what {
                true => Err(format!("{what} failed")),
                false => Ok(()),
            }
        }
    }

    impl Hands for Spy {
        fn copy(&self, text: &str) -> Result<(), String> {
            self.note("copy", text)
        }
        fn open_url(&self, url: &str) -> Result<(), String> {
            self.note("url", url)
        }
        fn open_file(&self, path: &str) -> Result<(), String> {
            self.note("file", path)
        }
    }

    #[test]
    fn each_effect_goes_to_its_own_hand_and_a_failure_becomes_the_notice() {
        let table: Vec<(Effect, &str, &str, Option<&str>)> = vec![
            (Effect::Nothing, "", "", None),
            (Effect::Copy("42".to_owned()), "", "copy 42", None),
            (
                Effect::OpenUrl("https://a.b".to_owned()),
                "",
                "url https://a.b",
                None,
            ),
            (
                Effect::OpenFile("/tmp/x".to_owned()),
                "",
                "file /tmp/x",
                None,
            ),
            (
                Effect::OpenFile("/tmp/x".to_owned()),
                "file",
                "file /tmp/x",
                Some("file failed"),
            ),
            (
                Effect::Copy("42".to_owned()),
                "copy",
                "copy 42",
                Some("copy failed"),
            ),
        ];
        for (effect, fail, call, notice) in table {
            let spy = Spy {
                calls: RefCell::new(Vec::new()),
                fail,
            };
            let got = carry_out(&effect, &spy);
            assert_eq!(got.as_deref(), notice, "{effect:?}");
            let want: Vec<&str> = match call.is_empty() {
                true => vec![],
                false => vec![call],
            };
            assert_eq!(*spy.calls.borrow(), want, "{effect:?}");
        }
    }
}

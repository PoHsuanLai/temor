//! A folder's page (PLAN §7, v14): what is in a folder, as Finder lists it. Folders first, then
//! files, each by name as a person reads it (`a2` before `a10`, case aside); hidden names (a
//! leading `.`) are left out, as Finder leaves them. Pure: the edge reads the folder.

use std::cmp::Ordering;

use temor_schema::session::At;

use crate::history::{Zone, day_heading};

/// At most this many entries are listed; a bigger folder says how many more it holds.
pub const LISTED: usize = 500;

/// One thing in a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub folder: bool,
    /// When it last changed, where the file system says.
    pub modified: Option<At>,
}

/// `entries` as the page lists them, and how many more there were than it lists.
pub fn listed(entries: Vec<Entry>) -> (Vec<Entry>, usize) {
    let mut shown: Vec<Entry> = entries
        .into_iter()
        .filter(|entry| !entry.name.starts_with('.'))
        .collect();
    shown.sort_by(|a, b| {
        b.folder
            .cmp(&a.folder)
            .then_with(|| by_name(&a.name, &b.name))
    });
    let more = shown.len().saturating_sub(LISTED);
    shown.truncate(LISTED);
    (shown, more)
}

/// When an entry changed, seen on `today`: the time today or yesterday, else the day.
pub fn modified_words(at: At, zone: Zone, today: i64) -> String {
    let day = zone.day_of(at);
    match today - day {
        0 | 1 => format!("{} {}", day_heading(day, today), zone.time_of(at)),
        _ => day_heading(day, today),
    }
}

/// Names compared as a person reads them: runs of digits by their value, the rest without case.
fn by_name(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                let order = x
                    .trim_start_matches('0')
                    .len()
                    .cmp(&y.trim_start_matches('0').len())
                    .then_with(|| x.trim_start_matches('0').cmp(y.trim_start_matches('0')));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        run.push(c);
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, folder: bool) -> Entry {
        Entry {
            name: name.to_owned(),
            folder,
            modified: None,
        }
    }

    #[test]
    fn folders_come_first_then_files_by_name_as_read_and_hidden_ones_are_left_out() {
        let (shown, more) = listed(vec![
            entry("run10.log", false),
            entry("README.md", false),
            entry(".git", true),
            entry("src", true),
            entry("run2.log", false),
            entry("Assets", true),
            entry("a.txt", false),
        ]);
        let names: Vec<&str> = shown.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Assets",
                "src",
                "a.txt",
                "README.md",
                "run2.log",
                "run10.log"
            ]
        );
        assert_eq!(more, 0);
    }

    #[test]
    fn a_big_folder_lists_its_first_and_says_how_many_more() {
        let many = (0..LISTED + 7)
            .map(|n| entry(&format!("f{n}"), false))
            .collect();
        let (shown, more) = listed(many);
        assert_eq!((shown.len(), more), (LISTED, 7));
    }

    #[test]
    fn a_change_today_or_yesterday_says_its_time_and_an_older_one_its_day() {
        let day = 20_000_i64;
        let at = |day: i64, minutes: i64| At(day * 86_400_000 + minutes * 60_000);
        assert_eq!(
            modified_words(at(day, 9 * 60 + 5), Zone(0), day),
            "Today 09:05"
        );
        assert_eq!(
            modified_words(at(day - 1, 0), Zone(0), day),
            "Yesterday 00:00"
        );
        assert_eq!(
            modified_words(at(day - 3, 0), Zone(0), day),
            day_heading(day - 3, day)
        );
    }
}

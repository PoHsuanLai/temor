//! How the history page words what it lists (PLAN P23): sessions and commands by day, "Today",
//! "Yesterday", then the day's name and date; each session by what it ran or where, and when.
//! Pure: the window gives the time now and the local zone's offset.

use temor_schema::session::{At, Session};

const DAY_MS: i64 = 86_400_000;

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The local wall clock: how far the zone is ahead of UTC, in seconds (negative west of
/// Greenwich). The offset of now is used for every time, so a day before a change of daylight
/// time can read an hour off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Zone(pub i32);

impl Zone {
    fn local_ms(self, at: At) -> i64 {
        at.0 + i64::from(self.0) * 1000
    }

    /// The local day `at` falls on, counted from 1970-01-01.
    pub fn day_of(self, at: At) -> i64 {
        self.local_ms(at).div_euclid(DAY_MS)
    }

    /// The time of day of `at`: `09:05`.
    pub fn time_of(self, at: At) -> String {
        let minute = self.local_ms(at).rem_euclid(DAY_MS) / 60_000;
        format!("{:02}:{:02}", minute / 60, minute % 60)
    }
}

/// The heading over a day's sessions, seen on `today`: "Today", "Yesterday", "Thursday 23
/// October", and the year too when it is not this one.
pub fn day_heading(day: i64, today: i64) -> String {
    match today - day {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        _ => {
            let (year, month, date) = civil(day);
            let weekday = WEEKDAYS[usize::try_from((day + 4).rem_euclid(7)).unwrap_or(0)];
            let month = MONTHS[month];
            match civil(today).0 == year {
                true => format!("{weekday} {date} {month}"),
                false => format!("{weekday} {date} {month} {year}"),
            }
        }
    }
}

/// When a session ran: "10:20 – now" while it runs, "17:02 – 18:10" once it ended, and only its
/// start when it neither runs nor said it ended.
pub fn span_of(session: &Session, running: bool, zone: Zone) -> String {
    let start = zone.time_of(session.started);
    match (running, session.ended) {
        (true, _) => format!("{start} – now"),
        (false, Some(end)) => format!("{start} – {}", zone.time_of(end)),
        (false, None) => start,
    }
}

/// What a session is called: the program it was started to run, else the last part of its
/// folder (`~` for the home folder itself).
pub fn session_title(session: &Session, home: &str) -> String {
    if let Some(program) = session.program.as_deref().filter(|p| !p.is_empty()) {
        return program.to_owned();
    }
    let folder = session.folder.trim_end_matches('/');
    if folder.is_empty() {
        return "/".to_owned();
    }
    if folder == home.trim_end_matches('/') {
        return "~".to_owned();
    }
    folder.rsplit('/').next().unwrap_or(folder).to_owned()
}

/// `items` in runs of the same day, newest first as they come, each under its heading.
pub fn by_day<T>(
    items: impl IntoIterator<Item = T>,
    at: impl Fn(&T) -> At,
    zone: Zone,
    today: i64,
) -> Vec<(String, Vec<T>)> {
    let mut days: Vec<(i64, Vec<T>)> = Vec::new();
    for item in items {
        let day = zone.day_of(at(&item));
        match days.last_mut() {
            Some((last, run)) if *last == day => run.push(item),
            _ => days.push((day, vec![item])),
        }
    }
    days.into_iter()
        .map(|(day, run)| (day_heading(day, today), run))
        .collect()
}

/// The civil date of `days` since 1970-01-01 (Howard Hinnant's `civil_from_days`); the month is
/// an index from 0.
fn civil(days: i64) -> (i64, usize, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let date = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, usize::try_from(month - 1).unwrap_or(0), date)
}

#[cfg(test)]
mod tests {
    use temor_schema::session::SessionId;
    use temor_schema::url::Host;

    use super::*;

    /// 2026-10-27 10:20 UTC, a Tuesday.
    const NOW: At = At(1_793_096_400_000);

    fn session(program: Option<&str>, folder: &str) -> Session {
        Session {
            id: SessionId::from_bits(1),
            host: Host::Local,
            folder: folder.to_owned(),
            program: program.map(str::to_owned),
            started: NOW,
            ended: None,
        }
    }

    #[test]
    fn days_read_today_yesterday_then_their_name_and_date() {
        let zone = Zone(0);
        let today = zone.day_of(NOW);
        assert_eq!(day_heading(today, today), "Today");
        assert_eq!(day_heading(today - 1, today), "Yesterday");
        assert_eq!(day_heading(today - 4, today), "Friday 23 October");
        assert_eq!(day_heading(today - 365, today), "Monday 27 October 2025");
    }

    #[test]
    fn the_zone_moves_the_time_and_the_day() {
        assert_eq!(Zone(0).time_of(NOW), "10:20");
        assert_eq!(Zone(8 * 3600).time_of(NOW), "18:20");
        let late = Zone(14 * 3600);
        assert_eq!(late.day_of(NOW), Zone(0).day_of(NOW) + 1);
        assert_eq!(Zone(-5 * 3600).time_of(At(0)), "19:00");
    }

    #[test]
    fn a_session_is_named_by_its_program_or_its_folder_and_said_when_it_ran() {
        let home = "/Users/po";
        assert_eq!(
            session_title(&session(Some("claude"), "/x"), home),
            "claude"
        );
        assert_eq!(session_title(&session(None, "/Users/po/ml/"), home), "ml");
        assert_eq!(session_title(&session(None, "/Users/po"), home), "~");
        assert_eq!(session_title(&session(None, "/"), home), "/");
        let mut ran = session(None, "/x");
        assert_eq!(span_of(&ran, true, Zone(0)), "10:20 – now");
        assert_eq!(span_of(&ran, false, Zone(0)), "10:20");
        ran.ended = Some(At(NOW.0 + 50 * 60_000));
        assert_eq!(span_of(&ran, false, Zone(0)), "10:20 – 11:10");
    }

    #[test]
    fn items_are_grouped_in_runs_of_one_day() {
        let zone = Zone(0);
        let today = zone.day_of(NOW);
        let ats = [
            NOW,
            At(NOW.0 - 3_600_000),
            At(NOW.0 - DAY_MS),
            At(NOW.0 - 3 * DAY_MS),
        ];
        let days = by_day(ats, |at| *at, zone, today);
        let headings: Vec<&str> = days.iter().map(|(h, _)| h.as_str()).collect();
        assert_eq!(headings, ["Today", "Yesterday", "Saturday 24 October"]);
        assert_eq!(days[0].1.len(), 2);
    }
}

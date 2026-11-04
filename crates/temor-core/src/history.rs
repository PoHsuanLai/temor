//! The calendar the history page lists by (PLAN P23): which local day a moment falls on, that
//! day's date, runs of items one day each, and what a session is called. Pure: the window gives
//! the local zone's offset, and words the days itself.

use temor_schema::session::Session;
use temor_schema::time::UnixMillis;

const DAY_MS: i64 = 86_400_000;

/// The local wall clock: how far the zone is ahead of UTC, in seconds (negative west of
/// Greenwich). The offset of now is used for every time, so a day before a change of daylight
/// time can read an hour off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Zone(pub i32);

impl Zone {
    fn local_ms(self, at: UnixMillis) -> i64 {
        at.0 + i64::from(self.0) * 1000
    }

    /// The local day `at` falls on, counted from 1970-01-01.
    pub fn day_of(self, at: UnixMillis) -> i64 {
        self.local_ms(at).div_euclid(DAY_MS)
    }

    /// The time of day of `at`: `09:05`.
    pub fn time_of(self, at: UnixMillis) -> String {
        let minute = self.local_ms(at).rem_euclid(DAY_MS) / 60_000;
        format!("{:02}:{:02}", minute / 60, minute % 60)
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

/// `items` in runs of the same local day, newest first as they come, each with its day.
pub fn by_day<T>(
    items: impl IntoIterator<Item = T>,
    at: impl Fn(&T) -> UnixMillis,
    zone: Zone,
) -> Vec<(i64, Vec<T>)> {
    let mut days: Vec<(i64, Vec<T>)> = Vec::new();
    for item in items {
        let day = zone.day_of(at(&item));
        match days.last_mut() {
            Some((last, run)) if *last == day => run.push(item),
            _ => days.push((day, vec![item])),
        }
    }
    days
}

/// A day on the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDay {
    pub year: i64,
    /// From 1 for January.
    pub month: u8,
    pub date: u8,
    /// From 0 for Sunday.
    pub weekday: u8,
}

/// The calendar day of `day`, counted from 1970-01-01 (a Thursday).
pub fn civil_day(day: i64) -> CivilDay {
    let (year, month, date) = civil(day);
    let small = |n: i64| u8::try_from(n).unwrap_or(0);
    CivilDay {
        year,
        month: small(i64::try_from(month).unwrap_or(0) + 1),
        date: small(date),
        weekday: small((day + 4).rem_euclid(7)),
    }
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
    const NOW: UnixMillis = UnixMillis(1_793_096_400_000);

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
    fn a_day_has_its_date_and_weekday() {
        let today = Zone(0).day_of(NOW);
        let day = |year, month, date, weekday| CivilDay {
            year,
            month,
            date,
            weekday,
        };
        assert_eq!(civil_day(today), day(2026, 10, 27, 2));
        assert_eq!(civil_day(today - 4), day(2026, 10, 23, 5));
        assert_eq!(civil_day(today - 365), day(2025, 10, 27, 1));
        assert_eq!(civil_day(0), day(1970, 1, 1, 4));
    }

    #[test]
    fn the_zone_moves_the_time_and_the_day() {
        assert_eq!(Zone(0).time_of(NOW), "10:20");
        assert_eq!(Zone(8 * 3600).time_of(NOW), "18:20");
        let late = Zone(14 * 3600);
        assert_eq!(late.day_of(NOW), Zone(0).day_of(NOW) + 1);
        assert_eq!(Zone(-5 * 3600).time_of(UnixMillis(0)), "19:00");
    }

    #[test]
    fn a_session_is_named_by_its_program_or_its_folder() {
        let home = "/Users/po";
        assert_eq!(
            session_title(&session(Some("claude"), "/x"), home),
            "claude"
        );
        assert_eq!(session_title(&session(None, "/Users/po/ml/"), home), "ml");
        assert_eq!(session_title(&session(None, "/Users/po"), home), "~");
        assert_eq!(session_title(&session(None, "/"), home), "/");
    }

    #[test]
    fn items_are_grouped_in_runs_of_one_day() {
        let zone = Zone(0);
        let today = zone.day_of(NOW);
        let ats = [
            NOW,
            UnixMillis(NOW.0 - 3_600_000),
            UnixMillis(NOW.0 - DAY_MS),
            UnixMillis(NOW.0 - 3 * DAY_MS),
        ];
        let days = by_day(ats, |at| *at, zone);
        let starts: Vec<i64> = days.iter().map(|(day, _)| *day).collect();
        assert_eq!(starts, [today, today - 1, today - 3]);
        assert_eq!(days[0].1.len(), 2);
    }
}

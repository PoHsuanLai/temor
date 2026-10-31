//! How the window words a moment: a day's heading ("Today", "Yesterday", then its name and
//! date), when a session ran, and when a file changed. The calendar is the model's
//! (`temor_client::model::history`); the words are the window's.

use temor_client::model::history::{Zone, civil_day};
use temor_schema::session::Session;
use temor_schema::time::UnixMillis;

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

/// The heading over a day's sessions, seen on `today`: "Today", "Yesterday", "Thursday 23
/// October", and the year too when it is not this one.
pub fn day_heading(day: i64, today: i64) -> String {
    match today - day {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        _ => {
            let civil = civil_day(day);
            let weekday = WEEKDAYS[usize::from(civil.weekday) % 7];
            let month = MONTHS[usize::from(civil.month.saturating_sub(1)) % 12];
            let date = civil.date;
            match civil_day(today).year == civil.year {
                true => format!("{weekday} {date} {month}"),
                false => format!("{weekday} {date} {month} {}", civil.year),
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

/// When a file changed, seen on `today`: the time today or yesterday, else the day.
pub fn modified_words(at: UnixMillis, zone: Zone, today: i64) -> String {
    let day = zone.day_of(at);
    match today - day {
        0 | 1 => format!("{} {}", day_heading(day, today), zone.time_of(at)),
        _ => day_heading(day, today),
    }
}

#[cfg(test)]
mod tests {
    use temor_schema::session::SessionId;
    use temor_schema::url::Host;

    use super::*;

    /// 2026-10-27 10:20 UTC, a Tuesday.
    const NOW: UnixMillis = UnixMillis(1_793_096_400_000);

    #[test]
    fn days_read_today_yesterday_then_their_name_and_date() {
        let today = Zone(0).day_of(NOW);
        assert_eq!(day_heading(today, today), "Today");
        assert_eq!(day_heading(today - 1, today), "Yesterday");
        assert_eq!(day_heading(today - 4, today), "Friday 23 October");
        assert_eq!(day_heading(today - 365, today), "Monday 27 October 2025");
    }

    #[test]
    fn a_session_says_when_it_ran() {
        let mut ran = Session {
            id: SessionId::from_bits(1),
            host: Host::Local,
            folder: "/x".to_owned(),
            program: None,
            started: NOW,
            ended: None,
        };
        assert_eq!(span_of(&ran, true, Zone(0)), "10:20 – now");
        assert_eq!(span_of(&ran, false, Zone(0)), "10:20");
        ran.ended = Some(UnixMillis(NOW.0 + 50 * 60_000));
        assert_eq!(span_of(&ran, false, Zone(0)), "10:20 – 11:10");
    }

    #[test]
    fn a_change_today_or_yesterday_says_its_time_and_an_older_one_its_day() {
        let day = 20_000_i64;
        let at = |day: i64, minutes: i64| UnixMillis(day * 86_400_000 + minutes * 60_000);
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

//! The wall clock and the local time zone, for the pages that show days and times.
//!
//! The zone is read from the system's zone files once, when the client is made, by jiff, which
//! reads them itself: nothing in the process may read the C library's zone. Its offset is taken
//! at the moment asked about, so a time on either side of a change to or from summer time reads
//! as the clock on the wall read then.

use std::time::{SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use temor_core::history::Zone;
use temor_schema::session::At;

/// The clock on the wall, and the zone it is read in.
#[derive(Debug, Clone)]
pub struct LocalTime {
    zone: TimeZone,
}

impl LocalTime {
    /// The system's zone; UTC when it does not say.
    pub fn system() -> Self {
        Self {
            zone: TimeZone::try_system().unwrap_or(TimeZone::UTC),
        }
    }

    /// A zone `offset` seconds east of UTC all year, for a test.
    pub fn fixed(offset: i32) -> Self {
        Self {
            zone: Offset::from_seconds(offset).map_or(TimeZone::UTC, TimeZone::fixed),
        }
    }

    /// The wall clock now. A clock set before 1970 reads as the epoch.
    pub fn now(&self) -> At {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        At(i64::try_from(since).unwrap_or(i64::MAX))
    }

    /// The zone's offset at `at`.
    pub fn zone_at(&self, at: At) -> Zone {
        Timestamp::from_millisecond(at.0).map_or_else(
            |_| Zone::default(),
            |at| Zone(self.zone.to_offset(at).seconds()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_zone_is_its_offset_at_any_time() {
        let time = LocalTime::fixed(8 * 3600);
        assert_eq!(time.zone_at(At(0)), Zone(8 * 3600));
        assert_eq!(time.zone_at(time.now()), Zone(8 * 3600));
    }

    #[test]
    fn a_zone_with_summer_time_has_the_offset_of_the_moment_asked() {
        let Ok(zone) = TimeZone::get("Europe/London") else {
            return;
        };
        let time = LocalTime { zone };
        // 2026-01-15 and 2026-07-15, noon UTC.
        assert_eq!(time.zone_at(At(1_768_478_400_000)), Zone(0));
        assert_eq!(time.zone_at(At(1_784_116_800_000)), Zone(3600));
    }
}

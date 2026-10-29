//! Frecency for the bar's folders: how often and how lately a folder was opened, as one number.
//! Pure, with the time given: the edge reads the clock and keeps the counts.

use super::model::Frecency;
use ds_core::word::Word;

/// Seconds since the Unix epoch, as the edge's clock reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnixSecs(pub u64);

const DAY: u64 = 24 * 60 * 60;

/// How long ago a visit was, in the bands that set its weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Recency {
    /// Within the last 4 days.
    Days4,
    /// Within the last 2 weeks.
    Weeks2,
    /// Within the last month.
    Month,
    /// Within the last 3 months.
    Months3,
    /// Longer ago.
    Older,
}

impl Recency {
    /// The band an age in seconds falls in.
    pub fn of_age(age: u64) -> Recency {
        match age {
            a if a <= 4 * DAY => Recency::Days4,
            a if a <= 14 * DAY => Recency::Weeks2,
            a if a <= 31 * DAY => Recency::Month,
            a if a <= 90 * DAY => Recency::Months3,
            _ => Recency::Older,
        }
    }

    /// What each visit in this band is worth.
    pub fn weight(self) -> u32 {
        match self {
            Recency::Days4 => 100,
            Recency::Weeks2 => 70,
            Recency::Month => 50,
            Recency::Months3 => 30,
            Recency::Older => 10,
        }
    }
}

/// How many times a folder was opened and when it last was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Visits {
    /// How many times.
    pub count: u32,
    /// The last time.
    pub last: UnixSecs,
}

impl Visits {
    /// These visits plus one at `now`.
    pub fn visited(self, now: UnixSecs) -> Visits {
        Visits {
            count: self.count.saturating_add(1),
            last: now.max(self.last),
        }
    }

    /// The score at `now`: the count times the weight of the band the last visit is in. A last
    /// visit after `now` (a clock that went back) counts as just now.
    pub fn frecency(self, now: UnixSecs) -> Frecency {
        let band = Recency::of_age(now.0.saturating_sub(self.last.0));
        Frecency(self.count.saturating_mul(band.weight()))
    }
}

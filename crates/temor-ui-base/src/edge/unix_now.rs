//! The wall clock as seconds since the Unix epoch, for the bar's folder scores.

use std::time::{SystemTime, UNIX_EPOCH};

use temor_client::model::bar::UnixSecs;

/// Now. A clock set before 1970 reads as the epoch.
pub fn unix_now() -> UnixSecs {
    UnixSecs(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    )
}

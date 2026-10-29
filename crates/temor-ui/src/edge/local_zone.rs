//! The local time zone's offset from UTC, for the history page's days and times: asked of the
//! system's `date` once a run, as nothing in the process may read the C library's zone.

use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use temor_client::model::history::Zone;
use temor_schema::session::At;

/// The zone now; UTC when the system does not say.
pub fn local_zone() -> Zone {
    static ZONE: OnceLock<Zone> = OnceLock::new();
    *ZONE.get_or_init(|| {
        Command::new("/bin/date")
            .arg("+%z")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .and_then(|text| offset_of(text.trim()))
            .map_or_else(Zone::default, Zone)
    })
}

/// The wall clock now. A clock set before 1970 reads as the epoch.
pub fn wall_now() -> At {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    At(i64::try_from(since).unwrap_or(i64::MAX))
}

/// `+0800` or `-0530` as seconds.
fn offset_of(text: &str) -> Option<i32> {
    let (sign, digits) = match text.split_at_checked(1)? {
        ("+", digits) => (1, digits),
        ("-", digits) => (-1, digits),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_is_read_as_date_writes_it() {
        assert_eq!(offset_of("+0800"), Some(8 * 3600));
        assert_eq!(offset_of("-0530"), Some(-(5 * 3600 + 30 * 60)));
        assert_eq!(offset_of("+0000"), Some(0));
        assert_eq!(offset_of("UTC"), None);
        assert_eq!(offset_of(""), None);
    }
}

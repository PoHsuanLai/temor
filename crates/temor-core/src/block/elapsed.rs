//! How long a command ran, in words the person reads (PLAN §9 block hover).

/// A duration as the hover and the notices show it: "840 ms", "4.2 s", "1 min 5 s", "2 h 3 min".
pub fn format_elapsed(ms: u64) -> String {
    match ms {
        0..1000 => format!("{ms} ms"),
        1000..60_000 => format!("{}.{} s", ms / 1000, (ms % 1000) / 100),
        60_000..3_600_000 => format!("{} min {} s", ms / 60_000, (ms % 60_000) / 1000),
        _ => format!("{} h {} min", ms / 3_600_000, (ms % 3_600_000) / 60_000),
    }
}

#[cfg(test)]
mod tests {
    use super::format_elapsed;

    #[test]
    fn durations_read_in_the_largest_fitting_units() {
        assert_eq!(format_elapsed(0), "0 ms");
        assert_eq!(format_elapsed(840), "840 ms");
        assert_eq!(format_elapsed(4_249), "4.2 s");
        assert_eq!(format_elapsed(65_000), "1 min 5 s");
        assert_eq!(format_elapsed(7_380_000), "2 h 3 min");
    }
}

//! Unique names on collision: `report.pdf`, then `report (2).pdf`, `report (3).pdf`, and so on.
//! The extension is the part after the last dot, unless the dot starts the name.

/// The `n`th candidate for `desired`. `n` of 0 or 1 is the name itself.
pub fn candidate_name(desired: &str, n: usize) -> String {
    if n <= 1 {
        return desired.to_owned();
    }
    let (stem, ext) = split_extension(desired);
    format!("{stem} ({n}){ext}")
}

/// The first candidate that `taken` does not reject.
pub fn unique_name(desired: &str, taken: impl Fn(&str) -> bool) -> String {
    (1..)
        .map(|n| candidate_name(desired, n))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| desired.to_owned())
}

fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        _ => (name, ""),
    }
}

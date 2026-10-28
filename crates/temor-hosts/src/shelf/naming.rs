//! Names: what a sender's name becomes, and the numbered names used when one is taken.

/// The longest name kept, in bytes. File systems allow 255; room is left for " (99)".
const MAX_NAME: usize = 200;

/// A name that is safe to create in the folder: only the last path component, no control
/// characters, no leading dots, never empty, never longer than 200 bytes.
pub fn safe_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .filter(|c| !c.is_control() && *c != ':')
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        return "download".to_owned();
    }
    let mut end = trimmed.len().min(MAX_NAME);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].to_owned()
}

/// The name at `attempt`: 0 is the name itself, 1 is `report (2).pdf`, 2 is `report (3).pdf`.
/// The extension is what follows the last dot, unless that dot starts the name.
pub fn candidate_name(name: &str, attempt: u32) -> String {
    if attempt == 0 {
        return name.to_owned();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        _ => (name, ""),
    };
    format!("{stem} ({}){ext}", u64::from(attempt) + 1)
}

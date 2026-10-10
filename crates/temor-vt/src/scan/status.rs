//! A pure reader for OSC 7501 program status payloads, for the fake terminal (and any relay
//! that sees the bytes without libghostty). The real terminal gets the same reports parsed by
//! libghostty; this reader follows the same shape so a fake-driven test and a real run agree.
//!
//! # What this assumes
//!
//! PLAN §8.1 names the sequence but not its grammar, and the specification page is not in the
//! repository. The grammar here is the one libghostty's header documents by example
//! (`state=blocked:kind=permission:app=terraform:msg=QXBwbHk/`):
//!
//! - the payload is `7501;` then `key=value` fields separated by `:`;
//! - keys are `state` (required: idle, working, done, blocked, error, clear), `kind`
//!   (permission, question, auth; kept only for blocked), `progress` (0 to 100; kept only for
//!   working and blocked), `id`, `app`, and `title` and `msg` (standard base64 of UTF-8);
//! - an unknown key is ignored; an unknown state, a bad base64 value, or a field over its
//!   limit refuses the whole report (libghostty only passes reports that pass every check);
//! - decoded text loses control characters; limits are 256 bytes for `id` and `app` and 1024
//!   bytes decoded for `title` and `msg`;
//! - `7501;?` is the support query, not a report, and gives `None` here.

use crate::{Percent, ProgramState, ProgramStatus, ProgramWait};

const ID_MAX: usize = 256;
const TEXT_MAX: usize = 1024;

/// The report in `payload` (everything between `ESC ]` and the terminator), or `None` when it
/// is not a valid OSC 7501 report.
pub(crate) fn parse_program_status(payload: &[u8]) -> Option<ProgramStatus> {
    let text = std::str::from_utf8(payload).ok()?;
    let fields = text.strip_prefix("7501;")?;
    let mut state = None;
    let mut wait = ProgramWait::Unspecified;
    let mut progress = None;
    let mut out = ProgramStatus {
        state: ProgramState::Idle,
        wait,
        percent: None,
        record: String::new(),
        app: String::new(),
        title: String::new(),
        message: String::new(),
    };
    for field in fields.split(':') {
        let Some((key, value)) = field.split_once('=') else {
            continue;
        };
        match key {
            "state" => state = Some(parse_state(value)?),
            "kind" => {
                wait = match value {
                    "permission" => ProgramWait::Permission,
                    "question" => ProgramWait::Question,
                    "auth" => ProgramWait::Auth,
                    _ => ProgramWait::Unspecified,
                }
            }
            "progress" => progress = value.parse::<i64>().ok().and_then(Percent::new),
            "id" => out.record = short(value, ID_MAX)?,
            "app" => out.app = short(value, ID_MAX)?,
            "title" => out.title = decoded(value)?,
            "msg" => out.message = decoded(value)?,
            _ => {}
        }
    }
    let state = state?;
    let blocked = state == ProgramState::Blocked;
    out.state = state;
    out.wait = if blocked {
        wait
    } else {
        ProgramWait::Unspecified
    };
    out.percent = progress.filter(|_| blocked || state == ProgramState::Working);
    Some(out)
}

fn parse_state(value: &str) -> Option<ProgramState> {
    Some(match value {
        "idle" => ProgramState::Idle,
        "working" => ProgramState::Working,
        "done" => ProgramState::Done,
        "blocked" => ProgramState::Blocked,
        "error" => ProgramState::Error,
        "clear" => ProgramState::Clear,
        _ => return None,
    })
}

fn short(value: &str, max: usize) -> Option<String> {
    (value.len() <= max).then(|| value.chars().filter(|c| !c.is_control()).collect())
}

fn decoded(value: &str) -> Option<String> {
    let bytes = base64(value)?;
    if bytes.len() > TEXT_MAX {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    Some(text.chars().filter(|c| !c.is_control()).collect())
}

/// Standard base64, padding optional.
fn base64(value: &str) -> Option<Vec<u8>> {
    let trimmed = value.trim_end_matches('=');
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in trimmed.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::parse_program_status;
    use crate::{Percent, ProgramState, ProgramWait};

    #[test]
    fn reports_follow_the_documented_example_and_the_rules() {
        let blocked =
            parse_program_status(b"7501;state=blocked:kind=permission:app=terraform:msg=QXBwbHk/")
                .expect("a report");
        assert_eq!(blocked.state, ProgramState::Blocked);
        assert_eq!(blocked.wait, ProgramWait::Permission);
        assert_eq!(
            (blocked.app.as_str(), blocked.message.as_str()),
            ("terraform", "Apply?")
        );
        assert_eq!(blocked.percent, None);

        let working =
            parse_program_status(b"7501;state=working:progress=30:id=a/b").expect("report");
        assert_eq!(working.percent, Percent::new(30));
        assert_eq!(working.record, "a/b");

        // kind only counts for blocked, progress only for working and blocked, unknown keys pass.
        let done =
            parse_program_status(b"7501;state=done:kind=auth:progress=5:x=1").expect("report");
        assert_eq!((done.wait, done.percent), (ProgramWait::Unspecified, None));
        let high = parse_program_status(b"7501;state=working:progress=101").expect("report");
        assert_eq!(high.percent, None);
    }

    #[test]
    fn invalid_reports_are_refused_whole() {
        for bad in [
            &b"7501;?"[..],
            b"7501;app=x",
            b"7501;state=sleeping",
            b"7501;state=idle:msg=!!!",
            b"7501;state=idle:msg=/w==",
            b"9;state=idle",
        ] {
            assert_eq!(
                parse_program_status(bad),
                None,
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        let long = format!("7501;state=idle:id={}", "x".repeat(300));
        assert_eq!(parse_program_status(long.as_bytes()), None);
    }
}

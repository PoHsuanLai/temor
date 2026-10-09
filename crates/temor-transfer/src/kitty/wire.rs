//! The wire format: `ESC ] 5113 ; key=value ; ... ST`. Keys may be written in short or long
//! form; unknown keys are ignored, as the protocol asks.

use super::model::{
    Action, Compression, ErrorCode, FileId, FileType, KittyId, Message, Mode, Reply, ReplyStatus,
    Transmission, UnixNanos,
};
use crate::base64;
use crate::error::{Malformed, TransferError};
use crate::units::{Bytes, Limits};

const PREFIX: &[u8] = b"5113;";
const OSC: &[u8] = b"\x1b]";
const ST: &[u8] = b"\x1b\\";
const MAX_NAME_TEXT: u64 = 4096;

/// Splits a payload into `(key, value)` pairs. Pairs with no `=` are skipped.
fn pairs(rest: &[u8]) -> Vec<(&[u8], &[u8])> {
    rest.split(|&byte| byte == b';')
        .filter_map(|pair| {
            let eq = pair.iter().position(|&byte| byte == b'=')?;
            Some((&pair[..eq], &pair[eq + 1..]))
        })
        .collect()
}

/// The last value given for any of `names` (the long and short forms of one key).
fn find<'a>(pairs: &[(&'a [u8], &'a [u8])], names: &[&str]) -> Option<&'a [u8]> {
    pairs
        .iter()
        .rev()
        .find(|(key, _)| names.iter().any(|name| name.as_bytes() == *key))
        .map(|(_, value)| *value)
}

fn ascii<'a>(value: &'a [u8], key: &'static str) -> Result<&'a str, TransferError> {
    std::str::from_utf8(value)
        .ok()
        .filter(|text| text.bytes().all(|byte| byte.is_ascii_graphic()))
        .ok_or_else(|| Malformed::BadValue { key }.into())
}

fn optional<T>(
    pairs: &[(&[u8], &[u8])],
    names: &[&str],
    parse: impl FnOnce(&[u8]) -> Result<T, TransferError>,
) -> Result<Option<T>, TransferError> {
    find(pairs, names).map(parse).transpose()
}

fn word<W: std::str::FromStr>(value: &[u8], key: &'static str) -> Result<W, TransferError> {
    ascii(value, key)?
        .parse()
        .map_err(|_| Malformed::BadValue { key }.into())
}

fn number<T: std::str::FromStr>(value: &[u8], key: &'static str) -> Result<T, TransferError> {
    ascii(value, key)?
        .parse()
        .map_err(|_| Malformed::BadValue { key }.into())
}

/// Parses one payload (the bytes between `ESC ]` and the terminator). The action and id are
/// required; every other field is optional. Data and names are decoded from base64 here,
/// under the caller's limits.
pub fn parse_message(payload: &[u8], limits: Limits) -> Result<Message, TransferError> {
    if payload.len() as u64 > limits.max_raw().get() {
        return Err(TransferError::TooLarge {
            limit: limits.max_bytes(),
        });
    }
    let rest = payload
        .strip_prefix(PREFIX)
        .ok_or(Malformed::NotATransfer)?;
    let pairs = pairs(rest);
    let action_raw =
        find(&pairs, &["ac", "action"]).ok_or(Malformed::MissingField { key: "ac" })?;
    let action = Action::from_wire(ascii(action_raw, "ac")?);
    let id_raw = find(&pairs, &["id"]).ok_or(Malformed::MissingField { key: "id" })?;
    let id = KittyId::parse(ascii(id_raw, "id")?)?;
    Ok(Message {
        action,
        id,
        file: optional(&pairs, &["fid", "file_id"], |v| {
            FileId::parse(ascii(v, "fid")?).map_err(TransferError::from)
        })?,
        parent: optional(&pairs, &["pr", "parent"], |v| {
            FileId::parse(ascii(v, "pr")?).map_err(TransferError::from)
        })?,
        name: optional(&pairs, &["n", "name"], |v| {
            let raw = base64::decode_capped(v, Bytes::new(MAX_NAME_TEXT))?;
            String::from_utf8(raw).map_err(|_| Malformed::BadValue { key: "n" }.into())
        })?,
        file_type: optional(&pairs, &["ft", "file_type", "ftype"], |v| {
            word::<FileType>(v, "ft")
        })?,
        size: optional(&pairs, &["sz", "size"], |v| {
            Ok(Bytes::new(number(v, "sz")?))
        })?,
        mtime: optional(&pairs, &["mod", "mtime"], |v| {
            Ok(UnixNanos::new(number(v, "mod")?))
        })?,
        permissions: optional(&pairs, &["prm", "permissions"], |v| {
            Ok(Mode::new(number(v, "prm")?))
        })?,
        compression: optional(&pairs, &["zip", "compression"], |v| {
            word::<Compression>(v, "zip")
        })?,
        transmission: optional(&pairs, &["tt", "transmission"], |v| {
            let text = ascii(v, "tt")?;
            Transmission::from_wire(text).ok_or_else(|| Malformed::BadValue { key: "tt" }.into())
        })?,
        data: optional(&pairs, &["d", "data"], |v| {
            base64::decode_capped(v, limits.max_bytes())
        })?
        .unwrap_or_default(),
    })
}

/// Parses a status reply (`ac=status`): the id, the file it is about, and the status text. The
/// status is base64 in `st`: `OK`, `STARTED`, `CANCELED` or `CODE:message`. A code this crate
/// does not know reads as `EIO` with the whole text as its message.
pub fn parse_reply(payload: &[u8], limits: Limits) -> Result<Reply, TransferError> {
    if payload.len() as u64 > limits.max_raw().get() {
        return Err(TransferError::TooLarge {
            limit: limits.max_bytes(),
        });
    }
    let rest = payload
        .strip_prefix(PREFIX)
        .ok_or(Malformed::NotATransfer)?;
    let pairs = pairs(rest);
    let action = find(&pairs, &["ac", "action"]).ok_or(Malformed::MissingField { key: "ac" })?;
    if Action::from_wire(ascii(action, "ac")?) != Action::Status {
        return Err(Malformed::NotATransfer.into());
    }
    let id_raw = find(&pairs, &["id"]).ok_or(Malformed::MissingField { key: "id" })?;
    let id = KittyId::parse(ascii(id_raw, "id")?)?;
    let file = optional(&pairs, &["fid", "file_id"], |v| {
        FileId::parse(ascii(v, "fid")?).map_err(TransferError::from)
    })?;
    let raw = find(&pairs, &["st", "status"]).ok_or(Malformed::MissingField { key: "st" })?;
    let text = base64::decode_capped(raw, Bytes::new(MAX_NAME_TEXT))?;
    let text = String::from_utf8_lossy(&text).into_owned();
    Ok(Reply {
        id,
        file,
        status: parse_status(&text),
    })
}

fn parse_status(text: &str) -> ReplyStatus {
    match text {
        "OK" => ReplyStatus::Ok,
        "STARTED" => ReplyStatus::Started,
        "CANCELED" => ReplyStatus::Cancelled,
        other => {
            let known = other.split_once(':').and_then(|(code, message)| {
                let code = code.to_lowercase().parse::<ErrorCode>().ok()?;
                Some((code, message))
            });
            match known {
                Some((code, message)) => ReplyStatus::Failed {
                    code,
                    message: message.to_owned(),
                },
                None => ReplyStatus::Failed {
                    code: ErrorCode::Eio,
                    message: other.to_owned(),
                },
            }
        }
    }
}

/// The id of a payload that does not parse, when its action and id are both readable. A frame
/// like that is answered for its transfer (with `EINVAL`) rather than dropped in silence.
pub fn readable_header(payload: &[u8], limits: Limits) -> Option<KittyId> {
    if payload.len() as u64 > limits.max_raw().get() {
        return None;
    }
    let rest = payload.strip_prefix(PREFIX)?;
    let pairs = pairs(rest);
    find(&pairs, &["ac", "action"])?;
    let raw = std::str::from_utf8(find(&pairs, &["id"])?).ok()?;
    KittyId::parse(raw).ok()
}

/// Encodes a message as a complete OSC sequence. Used to build sender-side frames and in tests.
pub fn encode_message(message: &Message) -> Vec<u8> {
    let mut fields = vec![
        format!("ac={}", message.action),
        format!("id={}", message.id.as_str()),
    ];
    if let Some(file) = &message.file {
        fields.push(format!("fid={}", file.as_str()));
    }
    if let Some(parent) = &message.parent {
        fields.push(format!("pr={}", parent.as_str()));
    }
    if let Some(name) = &message.name {
        fields.push(format!("n={}", base64::encode(name.as_bytes())));
    }
    if let Some(file_type) = message.file_type {
        fields.push(format!("ft={file_type}"));
    }
    if let Some(size) = message.size {
        fields.push(format!("sz={}", size.get()));
    }
    if let Some(mtime) = message.mtime {
        fields.push(format!("mod={}", mtime.get()));
    }
    if let Some(mode) = message.permissions {
        fields.push(format!("prm={}", mode.get()));
    }
    if let Some(compression) = message.compression {
        fields.push(format!("zip={compression}"));
    }
    if let Some(transmission) = &message.transmission {
        fields.push(format!("tt={}", transmission.word()));
    }
    if !message.data.is_empty() {
        fields.push(format!("d={}", base64::encode(&message.data)));
    }
    frame(&fields.join(";"))
}

/// Encodes a status reply. The status text is base64 in the `st` field, since it can hold spaces.
pub fn encode_reply(reply: &Reply) -> Vec<u8> {
    let status = match &reply.status {
        ReplyStatus::Ok => "OK".to_owned(),
        ReplyStatus::Started => "STARTED".to_owned(),
        ReplyStatus::Cancelled => "CANCELED".to_owned(),
        ReplyStatus::Failed { code, message } => format!("{}:{message}", code.code()),
    };
    let mut fields = vec![
        format!("ac={}", Action::Status),
        format!("id={}", reply.id.as_str()),
    ];
    if let Some(file) = &reply.file {
        fields.push(format!("fid={}", file.as_str()));
    }
    fields.push(format!("st={}", base64::encode(status.as_bytes())));
    frame(&fields.join(";"))
}

fn frame(body: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(OSC.len() + PREFIX.len() + body.len() + ST.len());
    out.extend_from_slice(OSC);
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(body.as_bytes());
    out.extend_from_slice(ST);
    out
}

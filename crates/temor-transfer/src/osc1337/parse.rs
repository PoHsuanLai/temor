//! Parses one OSC 1337 payload (the bytes between `ESC ]` and the terminator) into a command.

use crate::base64;
use crate::fields::{MAX_NAME_TEXT, ascii, pairs, parse_value};
use crate::error::{Malformed, TransferError};
use crate::name::FileName;
use crate::units::{Bytes, Limits};

use super::model::{
    Aspect, Completeness, Delivery, Dimension, FileHeader, MimeType, Osc1337Command,
};

const PREFIX: &[u8] = b"1337;";

/// Parses one payload. A cut payload is refused outright: its tail is missing, so nothing in it
/// can be trusted.
pub(crate) fn parse_command(
    payload: &[u8],
    completeness: Completeness,
    limits: Limits,
) -> Result<Osc1337Command, TransferError> {
    if completeness == Completeness::Cut {
        return Err(TransferError::Cut);
    }
    if payload.len() as u64 > limits.max_raw().get() {
        return Err(TransferError::TooLarge {
            limit: limits.max_bytes(),
        });
    }
    let rest = payload
        .strip_prefix(PREFIX)
        .ok_or(Malformed::NotATransfer)?;
    if let Some(args) = rest.strip_prefix(b"File=") {
        return parse_whole(args, limits);
    }
    if let Some(args) = rest.strip_prefix(b"MultipartFile=") {
        return Ok(Osc1337Command::MultipartStart(parse_header(args)?));
    }
    if let Some(text) = rest.strip_prefix(b"FilePart=") {
        return Ok(Osc1337Command::Part(text.to_vec()));
    }
    if rest == b"FileEnd" {
        return Ok(Osc1337Command::End);
    }
    Err(Malformed::NotATransfer.into())
}

fn parse_whole(args: &[u8], limits: Limits) -> Result<Osc1337Command, TransferError> {
    let colon = args
        .iter()
        .position(|&byte| byte == b':')
        .ok_or(Malformed::NoSeparator)?;
    let header = parse_header(&args[..colon])?;
    let data = base64::decode_capped(&args[colon + 1..], limits.max_bytes())?;
    if let Some(declared) = header.declared_size {
        let actual = Bytes::new(data.len() as u64);
        if declared != actual {
            return Err(Malformed::SizeMismatch { declared, actual }.into());
        }
    }
    Ok(Osc1337Command::Whole { header, data })
}

/// Parses `key=value;key=value` header arguments. Unknown keys are ignored (iTerm2 adds keys);
/// a known key with a bad value is an error.
fn parse_header(args: &[u8]) -> Result<FileHeader, TransferError> {
    let mut header = FileHeader::new();
    for (key, value) in pairs(args) {
        header = apply_key(header, key, value)?;
    }
    Ok(header)
}

fn apply_key(
    mut header: FileHeader,
    key: &[u8],
    value: &[u8],
) -> Result<FileHeader, TransferError> {
    match key {
        b"name" => {
            let raw = base64::decode(value, Bytes::new(MAX_NAME_TEXT))
                .map_err(|_| Malformed::BadValue { key: "name" })?;
            header.name = FileName::sanitise(&String::from_utf8_lossy(&raw));
        }
        b"size" => {
            header.declared_size = Some(Bytes::new(parse_value(value, "size")?));
        }
        b"inline" => {
            header.delivery = match ascii(value, "inline")? {
                "0" => Delivery::Offer,
                "1" => Delivery::Inline,
                _ => return Err(Malformed::BadValue { key: "inline" }.into()),
            };
        }
        b"width" => header.width = dimension(value, "width")?,
        b"height" => header.height = dimension(value, "height")?,
        b"preserveAspectRatio" => {
            header.aspect = match ascii(value, "preserveAspectRatio")? {
                "0" => Aspect::Stretch,
                "1" => Aspect::Preserve,
                _ => {
                    return Err(Malformed::BadValue {
                        key: "preserveAspectRatio",
                    }
                    .into());
                }
            };
        }
        b"type" => {
            let raw = parse_type(value)?;
            header.mime = Some(raw);
        }
        _ => {}
    }
    Ok(header)
}

fn parse_type(value: &[u8]) -> Result<MimeType, TransferError> {
    let text = ascii(value, "type")?;
    MimeType::parse(text).ok_or_else(|| Malformed::BadValue { key: "type" }.into())
}

/// `auto`, `N` (cells), `Npx` (pixels) or `N%` (percent of the pane, up to 100).
fn dimension(value: &[u8], key: &'static str) -> Result<Dimension, TransferError> {
    let text = ascii(value, key)?;
    let bad = || TransferError::from(Malformed::BadValue { key });
    if text == "auto" {
        return Ok(Dimension::Auto);
    }
    if let Some(number) = text.strip_suffix("px") {
        return number.parse().map(Dimension::Pixels).map_err(|_| bad());
    }
    if let Some(number) = text.strip_suffix('%') {
        let percent: u32 = number.parse().map_err(|_| bad())?;
        return if percent <= 100 {
            Ok(Dimension::Percent(percent))
        } else {
            Err(bad())
        };
    }
    text.parse().map(Dimension::Cells).map_err(|_| bad())
}

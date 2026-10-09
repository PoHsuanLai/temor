//! Standard base64, decoded with an output cap so hostile input cannot force a large allocation.
//! ASCII whitespace is skipped, as terminals and multi-line payloads may wrap it.

use crate::error::{Malformed, TransferError};
use crate::units::Bytes;

/// Why a base64 payload did not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
    Invalid,
    TooLarge,
}

fn sextet(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes `input`, failing once the output would exceed `cap`.
pub(crate) fn decode(input: &[u8], cap: Bytes) -> Result<Vec<u8>, DecodeError> {
    let body: Vec<u8> = input
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let data_len = body
        .iter()
        .rposition(|&byte| byte != b'=')
        .map_or(0, |i| i + 1);
    let pad = body.len() - data_len;
    let digits = &body[..data_len];
    if pad > 2 || digits.len() % 4 == 1 || (pad > 0 && !body.len().is_multiple_of(4)) {
        return Err(DecodeError::Invalid);
    }
    // Allocate from the cap, never from the input's length alone.
    let wanted = digits.len() / 4 * 3 + 3;
    let room = usize::try_from(cap.get()).unwrap_or(usize::MAX);
    let mut out = Vec::with_capacity(wanted.min(room));
    for group in digits.chunks(4) {
        let mut acc: u32 = 0;
        for &byte in group {
            acc = (acc << 6) | sextet(byte).ok_or(DecodeError::Invalid)?;
        }
        acc <<= 6 * (4 - group.len() as u32);
        let produced = group.len() - 1;
        for shift in 0..produced {
            // `shift` is below 3, so the byte index is valid.
            let byte = (acc >> (16 - 8 * shift)) & 0xff;
            out.push(byte as u8);
        }
        if out.len() as u64 > cap.get() {
            return Err(DecodeError::TooLarge);
        }
    }
    Ok(out)
}

/// Encodes `input` as padded standard base64.
pub(crate) fn encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let acc = (b0 << 16) | (b1 << 8) | b2;
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(
                    ALPHABET[((acc >> (18 - 6 * index)) & 0x3f) as usize],
                ));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes `text` under `cap`, mapping the decoder's failures onto the crate's error.
pub(crate) fn decode_capped(text: &[u8], cap: Bytes) -> Result<Vec<u8>, TransferError> {
    decode(text, cap).map_err(|err| match err {
        DecodeError::Invalid => Malformed::BadBase64.into(),
        DecodeError::TooLarge => TransferError::TooLarge { limit: cap },
    })
}

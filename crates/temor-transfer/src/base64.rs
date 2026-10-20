//! Standard base64, decoded with an output cap so hostile input cannot force a large allocation.
//! ASCII whitespace is skipped, as terminals and multi-line payloads may wrap it. The coding
//! itself is the `base64` crate's; this file adds the cap, the whitespace and the crate's errors.

use ::base64::Engine as _;
use ::base64::alphabet;
use ::base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig, general_purpose};

use crate::error::{Malformed, TransferError};
use crate::units::Bytes;

/// What a sender may write: padding or none, and spare bits in the last character ignored.
const READER: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// Why a base64 payload did not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
    Invalid,
    TooLarge,
}

/// Decodes `input`, failing before any output is made when it would exceed `cap`.
pub(crate) fn decode(input: &[u8], cap: Bytes) -> Result<Vec<u8>, DecodeError> {
    let body: Vec<u8> = input
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let digits = body
        .iter()
        .rposition(|&byte| byte != b'=')
        .map_or(0, |i| i + 1);
    // The decoded length, from the digits alone: three bytes a group, one or two for a short one.
    let wanted = digits / 4 * 3 + [0, 0, 1, 2][digits % 4];
    if wanted as u64 > cap.get() {
        return Err(DecodeError::TooLarge);
    }
    READER.decode(&body).map_err(|_| DecodeError::Invalid)
}

/// Encodes `input` as padded standard base64.
pub(crate) fn encode(input: &[u8]) -> String {
    general_purpose::STANDARD.encode(input)
}

/// Encodes `input` as standard base64 without the `=` padding, as kitty's file transfer
/// protocol writes it: its kitten refuses padded values ("illegal base64 data").
pub(crate) fn encode_unpadded(input: &[u8]) -> String {
    general_purpose::STANDARD_NO_PAD.encode(input)
}

/// Decodes `text` under `cap`, mapping the decoder's failures onto the crate's error.
pub(crate) fn decode_capped(text: &[u8], cap: Bytes) -> Result<Vec<u8>, TransferError> {
    decode(text, cap).map_err(|err| match err {
        DecodeError::Invalid => Malformed::BadBase64.into(),
        DecodeError::TooLarge => TransferError::TooLarge { limit: cap },
    })
}

//! The `key=value` fields both file-transfer wire formats carry (kitty's OSC 5113 and iTerm2's
//! OSC 1337 `File=`): splitting them and reading a value as text or a number.

use std::str::FromStr;

use crate::error::{Malformed, TransferError};

/// The longest base64 text of a name or a status message, in bytes.
pub(crate) const MAX_NAME_TEXT: u64 = 4096;

/// Splits a payload on `;` into `(key, value)` pairs. A piece with no `=` is skipped.
pub(crate) fn pairs(rest: &[u8]) -> Vec<(&[u8], &[u8])> {
    rest.split(|&byte| byte == b';')
        .filter_map(|pair| {
            let eq = pair.iter().position(|&byte| byte == b'=')?;
            Some((&pair[..eq], &pair[eq + 1..]))
        })
        .collect()
}

/// `value` as printable ASCII text, or a bad-value error naming `key`.
pub(crate) fn ascii<'a>(value: &'a [u8], key: &'static str) -> Result<&'a str, TransferError> {
    std::str::from_utf8(value)
        .ok()
        .filter(|text| text.bytes().all(|byte| byte.is_ascii_graphic()))
        .ok_or_else(|| Malformed::BadValue { key }.into())
}

/// `value` as printable ASCII, parsed as a `T` (a number or a word of a closed set).
pub(crate) fn parse_value<T: FromStr>(value: &[u8], key: &'static str) -> Result<T, TransferError> {
    ascii(value, key)?
        .parse()
        .map_err(|_| Malformed::BadValue { key }.into())
}

#[cfg(test)]
mod tests {
    use super::{ascii, pairs, parse_value};

    #[test]
    fn pairs_split_on_semicolons_and_skip_pieces_without_an_equals() {
        let got = pairs(b"a=1;;b=2=3;c;=d");
        let want: Vec<(&[u8], &[u8])> = vec![(b"a", b"1"), (b"b", b"2=3"), (b"", b"d")];
        assert_eq!(got, want);
    }

    #[test]
    fn values_are_printable_ascii_and_parse_or_name_their_key() {
        assert_eq!(ascii(b"abc", "k").ok(), Some("abc"));
        for bad in [&b"a b"[..], b"\xff", b"a\n"] {
            assert!(ascii(bad, "k").is_err(), "{bad:?}");
        }
        assert_eq!(parse_value::<u64>(b"42", "sz").ok(), Some(42));
        assert!(parse_value::<u64>(b"-1", "sz").is_err());
        assert!(parse_value::<u64>(b"4 2", "sz").is_err());
    }
}

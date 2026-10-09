//! Standard base64 (RFC 4648, with padding), so the bootstrap travels as plain ASCII. The
//! alphabet holds no quote or backslash, so nothing in it needs quoting for any login shell.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` as base64 text.
pub(super) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        for shift in [18, 12, 6, 0] {
            // Each index is six bits, so it is below 64 and the lookup cannot fail.
            let index = usize::try_from((n >> shift) & 0x3f).unwrap_or(0);
            out.push(char::from(ALPHABET[index]));
        }
        if chunk.len() < 3 {
            let pad = 3 - chunk.len();
            out.truncate(out.len() - pad);
            out.push_str(&"=".repeat(pad));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn matches_rfc_4648_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }
}

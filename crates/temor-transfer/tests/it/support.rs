//! Helpers shared by the test modules.

use temor_transfer::{Bytes, Count, Limits};

/// Caps for tests: the byte cap is the argument; entry and open caps are generous.
pub fn limits(max: u64) -> Limits {
    Limits::new(Bytes::new(max), Count::new(100_000), Count::new(10_000))
}

/// Standard base64, for building inputs. Mirrors the crate's encoder; kept separate so the
/// tests do not depend on the code under test.
pub fn b64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let acc = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
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

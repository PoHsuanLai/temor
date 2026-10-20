//! Fixtures the tests build files and repositories from.

/// A recognisable, non-repeating pattern of `len` bytes, for a file whose parts must land in
/// order.
pub fn pattern(len: u32) -> Vec<u8> {
    (0..len)
        .map(|n| (n % 251) as u8 ^ (n / 4093) as u8)
        .collect()
}

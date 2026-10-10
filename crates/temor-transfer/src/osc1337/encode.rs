//! Builds an OSC 1337 `File=` sequence: what a program prints to send a file, and what the tests
//! and a fallback sender use.

use crate::base64;

use super::model::Delivery;

/// `ESC ] 1337 ; File=name=<base64>;size=<n>;inline=<0|1> : <base64> ESC \`.
pub fn encode_file(name: &str, data: &[u8], delivery: Delivery) -> Vec<u8> {
    let inline = match delivery {
        Delivery::Offer => 0,
        Delivery::Inline => 1,
    };
    let text = format!(
        "\x1b]1337;File=name={};size={};inline={inline}:{}\x1b\\",
        base64::encode(name.as_bytes()),
        data.len(),
        base64::encode(data),
    );
    text.into_bytes()
}

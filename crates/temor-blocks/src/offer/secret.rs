//! Lines that ask the person for something secret or a yes or no: the offer never appears over
//! them, and a Yes never types into them.

/// The words of a prompt that wants a secret (or a decision about a host key).
const SECRET_WORDS: [&str; 7] = [
    "password",
    "passphrase",
    "passcode",
    "verification code",
    "one-time",
    "yes/no",
    "[y/n]",
];

/// Whether `line` (the last line on the screen) looks like such a prompt.
pub fn asks_for_secret(line: &str) -> bool {
    let line = line.to_lowercase();
    SECRET_WORDS.iter().any(|word| line.contains(word))
}

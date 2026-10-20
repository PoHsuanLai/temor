//! The protocol's closed word sets: `Display` writes a word and `FromStr` reads it back.

/// A word the protocol does not use for this set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a word of this set")]
pub struct UnknownWord(pub String);

/// The failure text a transfer the person turned down is answered with. Both ends of temor's own
/// transfers compare against it, so the sender can tell a "no" from a broken link.
pub const REFUSED_BY_PERSON: &str = "User refused the transfer";

/// `Display` writes a closed set's protocol word; `FromStr` reads it back.
macro_rules! wire_words {
    ($ty:ident { $($variant:ident => $word:literal),+ $(,)? }) => {
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self {
                    $(Self::$variant => $word,)+
                })
            }
        }

        impl std::str::FromStr for $ty {
            type Err = UnknownWord;

            fn from_str(text: &str) -> Result<Self, UnknownWord> {
                match text {
                    $($word => Ok(Self::$variant),)+
                    other => Err(UnknownWord(other.to_owned())),
                }
            }
        }
    };
}

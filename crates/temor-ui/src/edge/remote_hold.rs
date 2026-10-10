//! Input typed while a helper session has no link (PLAN §9b): it is held, in order, up to a
//! bound, and sent once the session is attached again. Past the bound the rest is dropped (never
//! a later piece ahead of an earlier one) and the pane says so.

/// How much typing a pane holds while its link is down.
pub const HOLD_LIMIT: usize = 64 * 1024;

/// Whether typing was lost while the link was down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Typing {
    /// Everything typed is held or sent.
    #[default]
    Kept,
    /// The hold filled up and what came after was dropped.
    Dropped,
}

/// What became of one piece of input offered to the hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Kept,
    /// Dropped; `first` is the piece that made the hold spill, the one the pane is told about.
    Dropped {
        first: bool,
    },
}

/// The held input.
#[derive(Debug, Default)]
pub struct Hold {
    bytes: Vec<u8>,
    typing: Typing,
}

impl Hold {
    /// Offers `input`. A piece that does not fit whole is dropped, and so is everything after
    /// it until the hold is taken.
    pub fn push(&mut self, input: &[u8]) -> Held {
        match (self.typing, self.bytes.len() + input.len() <= HOLD_LIMIT) {
            (Typing::Dropped, _) => Held::Dropped { first: false },
            (Typing::Kept, true) => {
                self.bytes.extend_from_slice(input);
                Held::Kept
            }
            (Typing::Kept, false) => {
                self.typing = Typing::Dropped;
                Held::Dropped { first: true }
            }
        }
    }

    /// Whether anything was lost since the hold was last taken.
    pub fn typing(&self) -> Typing {
        self.typing
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Takes what is held, in the order it came, and starts over.
    pub fn take(&mut self) -> Vec<u8> {
        self.typing = Typing::Kept;
        std::mem::take(&mut self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_is_held_in_order_up_to_the_bound_and_the_rest_is_dropped_once_told() {
        let cases = [
            ("under", vec![10, 10], Held::Kept, Typing::Kept, 20),
            (
                "exactly at",
                vec![HOLD_LIMIT],
                Held::Kept,
                Typing::Kept,
                HOLD_LIMIT,
            ),
            (
                "one past",
                vec![HOLD_LIMIT, 1],
                Held::Dropped { first: true },
                Typing::Dropped,
                HOLD_LIMIT,
            ),
            (
                "a piece too big for what is left",
                vec![HOLD_LIMIT - 4, 5],
                Held::Dropped { first: true },
                Typing::Dropped,
                HOLD_LIMIT - 4,
            ),
            (
                "a small piece after a spill is still dropped",
                vec![HOLD_LIMIT - 4, 5, 1],
                Held::Dropped { first: false },
                Typing::Dropped,
                HOLD_LIMIT - 4,
            ),
        ];
        for (name, pieces, last, typing, held) in cases {
            let mut hold = Hold::default();
            let mut got = Held::Kept;
            for len in pieces {
                got = hold.push(&vec![b'x'; len]);
            }
            assert_eq!(
                (got, hold.typing(), hold.take().len()),
                (last, typing, held),
                "{name}"
            );
        }
    }

    #[test]
    fn taking_gives_the_input_back_in_order_and_starts_over() {
        let mut hold = Hold::default();
        assert!(hold.is_empty());
        hold.push(b"ls ");
        hold.push(b"-l\r");
        hold.push(&vec![0; HOLD_LIMIT]);
        assert_eq!(hold.typing(), Typing::Dropped);
        assert_eq!(hold.take(), b"ls -l\r");
        assert!(hold.is_empty());
        assert_eq!(hold.typing(), Typing::Kept);
        assert_eq!(hold.push(b"again"), Held::Kept);
    }
}

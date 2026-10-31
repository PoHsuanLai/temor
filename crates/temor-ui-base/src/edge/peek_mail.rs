//! The hand-off between the peek worker's thread and the window (PLAN §10): the worker leaves the
//! card it made, the window takes it. Each ask has a ticket; an answer for an older ticket than
//! the latest is dropped, so a card for a file the pointer has left is never shown.

/// Which ask an answer belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(u64);

/// What the window finds when it looks for the card of a ticket.
#[derive(Debug, PartialEq)]
pub enum Taken<T> {
    /// Not made yet.
    Waiting,
    /// Made; it is handed over once.
    Ready(T),
    /// Another ask or a cancel came after this ticket.
    Stale,
}

/// The latest ask and its answer.
#[derive(Debug)]
pub struct Mail<T> {
    latest: u64,
    answer: Option<T>,
}

impl<T> Default for Mail<T> {
    fn default() -> Self {
        Mail {
            latest: 0,
            answer: None,
        }
    }
}

impl<T> Mail<T> {
    /// A new ask: the earlier ones, answered or not, are stale from now on.
    pub fn ask(&mut self) -> Ticket {
        self.latest += 1;
        self.answer = None;
        Ticket(self.latest)
    }

    /// Nothing is wanted any more.
    pub fn cancel(&mut self) {
        self.latest += 1;
        self.answer = None;
    }

    /// Whether `ticket` is still the latest ask (nothing came after it).
    pub fn is_latest(&self, ticket: Ticket) -> bool {
        ticket.0 == self.latest
    }

    /// The worker's answer for `ticket`; kept only if it is still the latest.
    pub fn deliver(&mut self, ticket: Ticket, answer: T) {
        if ticket.0 == self.latest {
            self.answer = Some(answer);
        }
    }

    /// What there is for `ticket`.
    pub fn take(&mut self, ticket: Ticket) -> Taken<T> {
        if ticket.0 != self.latest {
            return Taken::Stale;
        }
        match self.answer.take() {
            Some(answer) => Taken::Ready(answer),
            None => Taken::Waiting,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_taken_once_and_only_while_it_is_the_latest() {
        let mut mail = Mail::<&str>::default();
        let first = mail.ask();
        assert_eq!(mail.take(first), Taken::Waiting);
        let second = mail.ask();
        // The first file's card arrives late: dropped.
        mail.deliver(first, "late");
        assert_eq!(mail.take(first), Taken::Stale);
        assert_eq!(mail.take(second), Taken::Waiting);
        mail.deliver(second, "card");
        assert_eq!(mail.take(second), Taken::Ready("card"));
        assert_eq!(mail.take(second), Taken::Waiting);
        // A cancel makes the ask stale, answered or not.
        mail.deliver(second, "again");
        mail.cancel();
        assert_eq!(mail.take(second), Taken::Stale);
    }
}

//! What the dock shows on temor's icon: a count, a progress bar and a request for attention.

/// A number on the icon (running agents, commands waiting for a look).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BadgeCount(pub u32);

/// How far along something is, in whole percent. Always 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Progress(u8);

impl Progress {
    /// `percent`, held at 100 at most.
    pub fn percent(percent: u8) -> Self {
        Self(percent.min(100))
    }

    /// The whole percent.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Whether the icon should ask for the person's eye.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Attention {
    /// No.
    #[default]
    Calm,
    /// Yes: an agent is waiting for an answer.
    Needed,
}

/// Everything the dock shows. The default shows nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Badge {
    /// A count, or none.
    pub count: Option<BadgeCount>,
    /// A progress bar, or none.
    pub progress: Option<Progress>,
    /// Whether to ask for attention.
    pub attention: Attention,
}

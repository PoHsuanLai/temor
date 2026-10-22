//! Where a new tab and a split start (`shell.start_in`).

/// Where a command's paged output goes (`shell.pager`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PagerChoice {
    /// `git` and the like print straight into the command's block, which scrolls, folds and finds.
    #[default]
    Block,
    /// Programs run their own pager (`less`), on the full-screen terminal.
    System,
}

/// The folder a new tab or a split opens in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartIn {
    /// A new tab in the home folder, a split in the folder of the pane it came from.
    #[default]
    Usual,
    /// Both in the folder of the pane in front.
    Current,
    /// Both in the home folder.
    Home,
}

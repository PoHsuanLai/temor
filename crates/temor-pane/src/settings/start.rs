//! Where a new tab and a split start (`shell.start_in`).

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

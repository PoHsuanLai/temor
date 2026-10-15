//! What a terminal is created with.

use super::{ByteCount, CellPx, ColorScheme, GridSize, LineCount};

/// The settings a terminal starts with. Start from [`TerminalConfig::new`] and override fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalConfig {
    /// The grid in cells.
    pub size: GridSize,
    /// One cell in pixels; image placement and pixel reports use it.
    pub cell_px: CellPx,
    /// The scrollback lines to keep (libghostty prunes by whole pages, so a page more or fewer
    /// may be kept).
    pub scrollback: LineCount,
    /// The most bytes kitty images may use; zero turns kitty graphics off.
    pub kitty_image_storage: ByteCount,
    /// The most bytes of an unrecognised sequence (OSC 5113, APC, ...) to report.
    pub unknown_max: ByteCount,
    /// The most bytes of an inline file (OSC 1337 `File=`) to collect; longer ones are reported
    /// cut short.
    pub inline_file_max: ByteCount,
    /// The answer to a program asking for the colour scheme.
    pub color_scheme: ColorScheme,
}

impl TerminalConfig {
    /// Defaults for a `size` grid: 10,000 scrollback lines, 320 MB of kitty images (Ghostty's own
    /// default), 1 MiB per unrecognised sequence and 64 MiB per inline file.
    pub fn new(size: GridSize) -> Self {
        Self {
            size,
            cell_px: CellPx::new(8, 16),
            scrollback: LineCount(10_000),
            kitty_image_storage: ByteCount(320 * 1000 * 1000),
            unknown_max: ByteCount(1 << 20),
            inline_file_max: ByteCount(64 << 20),
            color_scheme: ColorScheme::Dark,
        }
    }
}

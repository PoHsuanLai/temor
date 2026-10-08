//! Text export: plain text, HTML or VT of the screen or a range.

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{TextFormat, TextSpan, VtError};

/// Exports terminal text. Soft-wrapped lines are joined and trailing blanks trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formatter {
    pub format: TextFormat,
}

impl Formatter {
    /// An exporter producing `format`.
    pub fn new(format: TextFormat) -> Self {
        Self { format }
    }

    /// The text of `span` in `term`.
    pub fn format(&self, term: &Terminal, span: TextSpan) -> Result<String, VtError> {
        let selection = match span {
            TextSpan::Screen => None,
            TextSpan::Range { start, end } => {
                let mut sel = temor_vt_sys::sized!(sys::Selection);
                sel.start = term.grid_ref(start)?;
                sel.end = term.grid_ref(end)?;
                Some(sel)
            }
        };
        let mut opts = temor_vt_sys::sized!(sys::FormatterTerminalOptions);
        opts.emit = match self.format {
            TextFormat::Plain => sys::FormatterFormat::PLAIN,
            TextFormat::Html => sys::FormatterFormat::HTML,
            TextFormat::Vt => sys::FormatterFormat::VT,
        };
        opts.unwrap = true;
        opts.trim = true;
        opts.extra.size = size_of::<sys::FormatterTerminalExtra>();
        opts.extra.screen.size = size_of::<sys::FormatterScreenExtra>();
        if self.format == TextFormat::Vt {
            opts.extra.screen.style = true;
            opts.extra.screen.hyperlink = true;
        }
        opts.selection = selection
            .as_ref()
            .map_or(std::ptr::null(), |s| s as *const sys::Selection);
        let mut formatter: sys::Formatter = std::ptr::null_mut();
        // SAFETY: the terminal is live; `opts` (and the selection it points to) outlive the call.
        check(
            unsafe {
                sys::ghostty_formatter_terminal_new(
                    std::ptr::null(),
                    &mut formatter,
                    term.raw(),
                    opts,
                )
            },
            "range",
        )?;
        let (mut ptr, mut len) = (std::ptr::null_mut(), 0usize);
        // SAFETY: the formatter is live; on success `ptr` holds `len` bytes from the default
        // allocator, copied and then freed with it. The formatter is freed on every path.
        unsafe {
            let made = check(
                sys::ghostty_formatter_format_alloc(
                    formatter,
                    std::ptr::null(),
                    &mut ptr,
                    &mut len,
                ),
                "format",
            );
            sys::ghostty_formatter_free(formatter);
            made?;
            let text = if ptr.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr, len).to_vec()
            };
            sys::ghostty_free(std::ptr::null(), ptr, len);
            Ok(String::from_utf8_lossy(&text).into_owned())
        }
    }
}

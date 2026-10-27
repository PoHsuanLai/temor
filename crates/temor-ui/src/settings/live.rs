//! What a change of settings means for the panes (PLAN §14). The font, the cursor and the notice
//! threshold apply to every pane that is open; the scrollback, the shell and the shell offer's
//! lists are read when a pane starts, so they reach the panes that come next. Pure.

use temor_panes::pane::settings::{
    CursorBlinking, CursorStyle, FontChoice, FontPoints, LinePercent, NotifyAfter, TerminalSettings,
};

/// What to apply to the open panes: each part is there only when it changed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveChange {
    pub font: Option<(FontChoice, FontPoints, LinePercent)>,
    pub cursor: Option<CursorStyle>,
    pub blink: Option<CursorBlinking>,
    pub notify: Option<NotifyAfter>,
}

impl LiveChange {
    /// Whether nothing needs applying to the open panes.
    pub fn is_none(&self) -> bool {
        *self == Self::default()
    }
}

/// What differs between the settings in force (`old`) and the ones just read (`new`) for the
/// panes that are open.
pub fn live_changes(old: &TerminalSettings, new: &TerminalSettings) -> LiveChange {
    LiveChange {
        font: ((&old.font, old.font_size, old.line_height)
            != (&new.font, new.font_size, new.line_height))
            .then(|| (new.font.clone(), new.font_size, new.line_height)),
        cursor: (old.cursor != new.cursor).then_some(new.cursor),
        blink: (old.cursor_blink != new.cursor_blink).then_some(new.cursor_blink),
        notify: (old.notify_after != new.notify_after).then_some(new.notify_after),
    }
}

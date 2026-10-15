//! The pages of the Settings window, named where the command bar and the keymap can reach them
//! without the window's toolkit: temor-pane knows only which page an action asks for.

/// One page of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SettingsPane {
    #[default]
    General,
    Appearance,
    Keyboard,
    Spaces,
}

impl SettingsPane {
    /// Every page, in the order the window's sidebar lists them.
    pub const ALL: [SettingsPane; 4] = [
        SettingsPane::General,
        SettingsPane::Appearance,
        SettingsPane::Keyboard,
        SettingsPane::Spaces,
    ];

    /// The page's name, in the sidebar.
    pub fn name(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Keyboard => "Keyboard",
            Self::Spaces => "Spaces",
        }
    }

    /// The command bar's row that opens the window on this page.
    pub fn row_label(self) -> &'static str {
        match self {
            Self::General => "Settings: General",
            Self::Appearance => "Settings: Appearance",
            Self::Keyboard => "Settings: Keyboard",
            Self::Spaces => "Settings: Spaces",
        }
    }
}

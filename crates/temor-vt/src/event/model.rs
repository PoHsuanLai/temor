//! The event enum and the payloads it carries.

use ds_core::word::Word;

use super::ProgramStatus;

/// Something the program in the terminal did that the host must act on. Events are sent from
/// inside `write` (and `resize`, which can end a render hold), on the thread that called it, in
/// the order the bytes produced them. Replies to the program do not travel here: they go to the
/// terminal's [`crate::ReplySink`], which `write` and `resize` may also call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtEvent {
    /// BEL.
    Bell,
    /// The window title changed (OSC 0/2); the new title. libghostty only lets the title be read
    /// once a write has returned, so this is sent once, after the write's other events, with the
    /// final title: several title changes in one write merge into one event.
    Title(String),
    /// The working directory changed (OSC 7 and friends), as the raw `file://host/path` URL.
    Cwd(String),
    /// The program wrote to a clipboard (OSC 52 and others). It was told the write succeeded;
    /// whether to perform it is the host's decision.
    Clipboard { write: ClipboardWrite },
    /// A desktop notification (OSC 9, OSC 777).
    Notification(Notification),
    /// A progress report (OSC 9;4).
    Progress(Progress),
    /// A program status report (OSC 7501).
    ProgramStatus(ProgramStatus),
    /// A shell integration mark (OSC 133 A/B/C/D).
    Prompt(PromptEvent),
    /// The program fully reset the terminal (RIS, `ESC c`). The title, working directory,
    /// modes, screen and scrollback are cleared without `Title` or `Cwd` events; forget what is
    /// known about the running command.
    Reset,
    /// The program held or released screen updates (synchronized output, mode 2026).
    RenderHold(RenderHold),
    /// The program asked for the colour scheme (CSI ? 996 n); it was answered with the
    /// configured one.
    ColorScheme,
    /// A sequence libghostty does not implement (OSC 5113 transfers, APC), or an inline file
    /// (OSC 1337 `File=`) taken out before libghostty saw it. `payload` is everything between the
    /// introducer (`ESC ]` or `ESC _`) and the terminator.
    Unknown {
        kind: UnknownKind,
        payload: Vec<u8>,
        truncation: Truncation,
    },
}

/// Which kind of sequence an [`VtEvent::Unknown`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum UnknownKind {
    /// An application program command (`ESC _ ... ST`).
    Apc,
    /// An operating system command libghostty does not implement (`ESC ] ... ST`).
    Osc,
    /// An iTerm2 inline file (`ESC ] 1337 ; File= ... BEL|ST`).
    Osc1337File,
}

/// Whether a payload is whole or was cut at the configured limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Truncation {
    Complete,
    Truncated,
}

/// Whether the screen is held for a synchronized update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum RenderHold {
    /// Keep showing the last frame; the program is drawing the next one.
    Held,
    /// Draw again.
    Released,
}

/// Which clipboard a write goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ClipboardLocation {
    Standard,
    Selection,
    Primary,
}

/// Whether the terminal already holds the person's permission for this program's writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ClipboardGrant {
    /// Write without asking.
    Granted,
    /// Ask (or apply the host's policy) first.
    Ask,
}

/// One representation of the value being copied.
#[derive(Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    pub mime: String,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for ClipboardItem {
    // Clipboard contents stay out of logs and panics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardItem")
            .field("mime", &self.mime)
            .field("data", &format_args!("<{} bytes>", self.data.len()))
            .finish()
    }
}

/// A clipboard write: every item is a representation of the same value. No items means clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardWrite {
    pub location: ClipboardLocation,
    pub items: Vec<ClipboardItem>,
    /// The program's name for permission prompts, when the protocol carries one.
    pub program: String,
    pub grant: ClipboardGrant,
}

/// A desktop notification. `title` is empty when the protocol has none (OSC 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// A percentage, 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Percent(u8);

impl Percent {
    /// The percentage, or `None` outside 0..=100.
    pub fn new(value: i64) -> Option<Self> {
        u8::try_from(value).ok().filter(|v| *v <= 100).map(Self)
    }

    /// The value, 0 to 100.
    pub fn value(self) -> u8 {
        self.0
    }
}

/// What a progress report says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ProgressState {
    /// Hide the progress.
    Remove,
    /// Show `percent`.
    Set,
    /// The work failed.
    Error,
    /// Busy, amount unknown.
    Indeterminate,
    /// Paused.
    Pause,
}

/// An OSC 9;4 progress report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Progress {
    pub state: ProgressState,
    /// The percentage, when the program sent one.
    pub percent: Option<Percent>,
}

/// Which prompt an OSC 133 A starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PromptKind {
    Primary,
    Right,
    Continuation,
    Secondary,
}

/// A command's exit code as the shell reported it (negative codes exist, on Windows say).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExitCode(pub i32);

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitStatus {
    /// The shell reported this exit code.
    Code(ExitCode),
    /// The shell did not say.
    Unreported,
}

/// One step of a command, as shell integration (OSC 133) reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptEvent {
    /// A: the prompt starts.
    PromptStart(PromptKind),
    /// B: the person's input starts.
    InputStart,
    /// C: the command's output starts. `command` is the command line when the shell sent it.
    OutputStart { command: String },
    /// D: the command ended. `error` is the shell's description, when it sent one.
    CommandEnd { exit: ExitStatus, error: String },
}

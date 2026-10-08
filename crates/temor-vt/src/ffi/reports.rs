//! The report callbacks: notifications, progress, program status and shell integration marks.

use std::ffi::c_void;

use temor_vt_sys as sys;

use super::callbacks::guarded;
use super::convert::string;
use crate::{
    ExitCode, ExitStatus, Notification, Percent, ProgramState, ProgramStatus, ProgramWait,
    Progress, ProgressState, PromptEvent, PromptKind, VtEvent,
};

pub(super) unsafe extern "C" fn on_notification(
    _t: sys::Terminal,
    ud: *mut c_void,
    n: *const sys::TerminalDesktopNotification,
) {
    if n.is_null() {
        return;
    }
    // SAFETY: `n` and its strings are valid for this call and copied here. `ud` is ours.
    unsafe {
        guarded(ud, (), |cb| {
            let n = &*n;
            let note = Notification {
                title: string(n.title),
                body: string(n.body),
            };
            cb.send(VtEvent::Notification(note));
        })
    }
}

pub(super) unsafe extern "C" fn on_progress(
    _t: sys::Terminal,
    ud: *mut c_void,
    p: *const sys::TerminalProgressReport,
) {
    if p.is_null() {
        return;
    }
    // SAFETY: `p` is valid for this call. `ud` is ours.
    unsafe { guarded(ud, (), |cb| cb.send_progress(&*p)) }
}

impl super::callbacks::Callbacks {
    fn send_progress(&self, p: &sys::TerminalProgressReport) {
        let state = match p.state {
            sys::TerminalProgressState::REMOVE => ProgressState::Remove,
            sys::TerminalProgressState::SET => ProgressState::Set,
            sys::TerminalProgressState::ERROR => ProgressState::Error,
            sys::TerminalProgressState::INDETERMINATE => ProgressState::Indeterminate,
            sys::TerminalProgressState::PAUSE => ProgressState::Pause,
            _ => return,
        };
        let percent = Percent::new(i64::from(p.progress));
        self.send(VtEvent::Progress(Progress { state, percent }));
    }
}

pub(super) unsafe extern "C" fn on_program_status(
    _t: sys::Terminal,
    ud: *mut c_void,
    p: *const sys::TerminalProgramStatus,
) {
    if p.is_null() {
        return;
    }
    // SAFETY: `p` and its strings are valid for this call. `ud` is ours.
    unsafe { guarded(ud, (), |cb| cb.send_status(&*p)) }
}

impl super::callbacks::Callbacks {
    /// # Safety
    /// The report's strings must be valid for the call.
    unsafe fn send_status(&self, p: &sys::TerminalProgramStatus) {
        let state = match p.state {
            sys::ProgramStatusState::IDLE => ProgramState::Idle,
            sys::ProgramStatusState::WORKING => ProgramState::Working,
            sys::ProgramStatusState::DONE => ProgramState::Done,
            sys::ProgramStatusState::BLOCKED => ProgramState::Blocked,
            sys::ProgramStatusState::ERROR => ProgramState::Error,
            sys::ProgramStatusState::CLEAR => ProgramState::Clear,
            _ => return,
        };
        let wait = match p.kind {
            sys::ProgramStatusKind::PERMISSION => ProgramWait::Permission,
            sys::ProgramStatusKind::QUESTION => ProgramWait::Question,
            sys::ProgramStatusKind::AUTH => ProgramWait::Auth,
            _ => ProgramWait::Unspecified,
        };
        // SAFETY: the strings are borrowed for this call and copied here.
        let status = unsafe {
            ProgramStatus {
                state,
                wait,
                percent: Percent::new(i64::from(p.progress)),
                record: string(p.id),
                app: string(p.app),
                title: string(p.title),
                message: string(p.message),
            }
        };
        self.send(VtEvent::ProgramStatus(status));
    }
}

pub(super) unsafe extern "C" fn on_prompt(
    _t: sys::Terminal,
    ud: *mut c_void,
    e: *const sys::TerminalSemanticPrompt,
) {
    if e.is_null() {
        return;
    }
    // SAFETY: `e` and its strings are valid for this call. `ud` is ours.
    unsafe { guarded(ud, (), |cb| cb.send_prompt(&*e)) }
}

impl super::callbacks::Callbacks {
    /// # Safety
    /// The event's strings must be valid for the call.
    unsafe fn send_prompt(&self, e: &sys::TerminalSemanticPrompt) {
        use sys::SemanticPromptKind as K;
        use sys::SemanticPromptPromptKind as P;
        let event = match e.kind {
            K::GHOSTTY_SEMANTIC_PROMPT_PROMPT_START => {
                PromptEvent::PromptStart(match e.prompt_kind {
                    P::GHOSTTY_SEMANTIC_PROMPT_PROMPT_RIGHT => PromptKind::Right,
                    P::GHOSTTY_SEMANTIC_PROMPT_PROMPT_CONTINUATION => PromptKind::Continuation,
                    P::GHOSTTY_SEMANTIC_PROMPT_PROMPT_SECONDARY => PromptKind::Secondary,
                    _ => PromptKind::Primary,
                })
            }
            K::GHOSTTY_SEMANTIC_PROMPT_INPUT_START => PromptEvent::InputStart,
            // SAFETY: `command` is borrowed for this call and copied here.
            K::GHOSTTY_SEMANTIC_PROMPT_OUTPUT_START => PromptEvent::OutputStart {
                command: unsafe { string(e.command) },
            },
            K::GHOSTTY_SEMANTIC_PROMPT_COMMAND_END => PromptEvent::CommandEnd {
                exit: if e.has_exit_code {
                    ExitStatus::Code(ExitCode(e.exit_code))
                } else {
                    ExitStatus::Unreported
                },
                // SAFETY: `error` is borrowed for this call and copied here.
                error: unsafe { string(e.error) },
            },
            _ => return,
        };
        self.send(VtEvent::Prompt(event));
    }
}

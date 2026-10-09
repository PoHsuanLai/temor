//! How a program ended, as the wire says it.

use temor_pty::ChildExit;
use temor_wire::message::ExitStatus;
use temor_wire::units::ExitCode;

/// How a program ended, as the wire says it. An unreadable status counts as a failed exit.
pub(super) fn exit_status(exit: Option<ChildExit>) -> ExitStatus {
    match exit {
        Some(ChildExit::Exited(code)) => ExitStatus::Code(ExitCode(code)),
        Some(ChildExit::Signaled(number)) => ExitStatus::Signal {
            name: signal_name(number),
        },
        None => ExitStatus::Code(ExitCode(-1)),
    }
}

fn signal_name(number: i32) -> String {
    let known = match number {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        6 => "SIGABRT",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        _ => return format!("SIG{number}"),
    };
    known.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exits_map_to_the_wire_and_unknown_signals_keep_their_number() {
        let cases = [
            (Some(ChildExit::Exited(7)), ExitStatus::Code(ExitCode(7))),
            (None, ExitStatus::Code(ExitCode(-1))),
            (
                Some(ChildExit::Signaled(9)),
                ExitStatus::Signal {
                    name: "SIGKILL".into(),
                },
            ),
            (
                Some(ChildExit::Signaled(31)),
                ExitStatus::Signal {
                    name: "SIG31".into(),
                },
            ),
        ];
        for (exit, want) in cases {
            assert_eq!(exit_status(exit), want, "{exit:?}");
        }
    }
}

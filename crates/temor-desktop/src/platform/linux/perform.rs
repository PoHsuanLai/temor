//! temor's actions: each reads its arguments, calls the window and shapes the answer. Anything
//! that came out of a terminal is untrusted and kept inside the Space it was read in.

use super::args::{Reader, failed, refusal};
use crate::{
    COMMAND_LIST, COMMAND_READ, COMMAND_RUN_IN_PANE, COMMAND_RUN_SANDBOXED, CommandInfo, Exit,
    Focus, PANE_CWD, PANE_LIST, PANE_OPEN, PANE_READ, PaneInfo, PaneKind, PaneOpen, PaneRef,
    SandboxRun, WindowPort,
};
use docket_core::{
    AppRefusal, Follow, Invocation, LabelText, Outcome, Preview, UndoToken, Undoable, Value,
};
use porter_core::{AppName, DataClass, SpaceId};
use prov::{Confidentiality, Effect, Integrity, Label, Labelled, Source};
use serde_json::json;
use std::collections::BTreeSet;

/// The most characters one answer carries; the manifest allows 65536.
const MOST_CHARS: usize = 60_000;

/// Run `inv` against the window.
pub(super) async fn perform<W: WindowPort>(
    window: &W,
    app: &AppName,
    inv: Invocation,
) -> Result<Outcome, AppRefusal> {
    let args = Reader::new(&inv.args);
    let front = || window.context().focused.map(|pane| pane.id);
    let shape = Shape {
        app,
        space: &inv.space,
    };
    match inv.action.as_str() {
        PANE_LIST => Ok(shape.read(pane_list(&window.panes()))),
        PANE_CWD => {
            let pane = args.pane(front())?;
            let found = window.panes().into_iter().find(|p| p.id == pane);
            let cwd = found
                .and_then(|p| p.cwd)
                .ok_or_else(|| failed("that pane has not reported a folder"))?;
            Ok(shape.read(cwd.to_string_lossy().into_owned()))
        }
        PANE_READ => {
            let pane = args.pane(front())?;
            let text = window
                .read_pane(pane, args.range()?)
                .await
                .map_err(refusal)?;
            Ok(shape.read(text))
        }
        COMMAND_LIST => {
            let commands = window.commands(args.pane(front())?).map_err(refusal)?;
            Ok(shape.read(command_list(&commands)))
        }
        COMMAND_READ => {
            let text = window
                .read_command(args.pane(front())?, args.command()?)
                .await
                .map_err(refusal)?;
            Ok(shape.read(text))
        }
        PANE_OPEN => {
            let kind = args.kind()?;
            let open = PaneOpen {
                kind,
                placement: args.placement()?,
                cwd: args.cwd()?,
            };
            let pane = window.open_pane(open).await.map_err(refusal)?;
            Ok(shape.opened(pane, kind))
        }
        COMMAND_RUN_SANDBOXED => {
            let run = SandboxRun {
                line: args.need_text("line")?.to_owned(),
                cwd: args.cwd()?,
                network_open: args.network()?,
            };
            let done = window.run_sandboxed(run).await.map_err(refusal)?;
            let status = match done.exit {
                Exit::Running => "still running".to_owned(),
                Exit::Code(code) => format!("exit status {code}"),
                Exit::Unknown => "ended, status not reported".to_owned(),
            };
            let mut outcome = shape.read(format!("{status}\n{}", done.output));
            if let Some(token) = done.undo {
                outcome.undo = token_of(&format!("{RUN_TOKEN}{token}"))?;
            }
            Ok(outcome)
        }
        COMMAND_RUN_IN_PANE => {
            let pane = args.pane(front())?;
            window
                .run_in_pane(pane, args.need_text("line")?)
                .await
                .map_err(refusal)?;
            Ok(shape.said("Typed the command into the pane", None))
        }
        _ => Err(AppRefusal::Unsupported),
    }
}

/// The effect one sandboxed call has: with the network closed the overlay is the undo.
pub(super) fn class_of(network: bool) -> Effect {
    match network {
        false => Effect::UndoableWrite,
        true => Effect::Outbound,
    }
}

/// The start of an undo token for a pane an agent opened.
const PANE_TOKEN: &str = "pane:";
/// The start of an undo token for a sandboxed run; the rest is the window's own.
pub(super) const RUN_TOKEN: &str = "run:";

/// The undo token for an opened pane.
pub(super) fn pane_token(pane: PaneRef) -> String {
    format!("{PANE_TOKEN}{}", pane.0)
}

/// The pane an undo token names, if it is a pane token.
pub(super) fn pane_of_token(token: &str) -> Option<PaneRef> {
    token.strip_prefix(PANE_TOKEN)?.parse().ok().map(PaneRef)
}

fn token_of(text: &str) -> Result<Undoable, AppRefusal> {
    UndoToken::parse(text)
        .map(Undoable::Yes)
        .map_err(|_| failed("could not make an undo token"))
}

/// The pane list as the JSON an agent reads.
pub(super) fn pane_list(panes: &[PaneInfo]) -> String {
    let rows: Vec<_> = panes
        .iter()
        .map(|pane| {
            json!({
                "pane": pane.id.0,
                "kind": pane.kind.word(),
                "title": pane.title,
                "folder": pane.cwd.as_ref().map(|cwd| cwd.to_string_lossy()),
                "in_front": matches!(pane.focus, Focus::Focused),
            })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// A pane's commands as the JSON an agent reads.
pub(super) fn command_list(commands: &[CommandInfo]) -> String {
    let rows: Vec<_> = commands
        .iter()
        .map(|command| {
            let exit = match command.exit {
                Exit::Running | Exit::Unknown => serde_json::Value::Null,
                Exit::Code(code) => json!(code),
            };
            json!({ "command": command.id.0, "line": command.line, "exit": exit })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// Builds outcomes with the labels this call's answers carry.
struct Shape<'a> {
    app: &'a AppName,
    space: &'a SpaceId,
}

impl Shape<'_> {
    /// Terminal text: untrusted, private to the Space it was read in.
    fn read(&self, text: String) -> Outcome {
        let text: String = text.chars().take(MOST_CHARS).collect();
        let label = Label::untrusted(
            Source::Screen {
                app: self.app.clone(),
            },
            DataClass::Screen,
            self.space.clone(),
        );
        Outcome {
            value: Some(Labelled::new(Value::Text(text), label)),
            said: None,
            show: Preview::None,
            undo: Undoable::No,
            follow: Follow::Nothing,
        }
    }

    fn said(&self, words: &str, value: Option<Labelled<Value>>) -> Outcome {
        Outcome {
            value,
            said: LabelText::parse(words).ok(),
            show: Preview::None,
            undo: Undoable::No,
            follow: Follow::Nothing,
        }
    }

    /// The new pane's number, with the token that closes it again.
    fn opened(&self, pane: PaneRef, kind: PaneKind) -> Outcome {
        let label = Label {
            integrity: Integrity::Trusted,
            confidentiality: Confidentiality::Public,
            classes: BTreeSet::from([DataClass::AppOwn]),
            sources: BTreeSet::from([Source::App(self.app.clone())]),
        };
        let number = i64::try_from(pane.0).unwrap_or(i64::MAX);
        let words = match kind {
            PaneKind::Terminal => "Opened a terminal",
            PaneKind::Agent => "Opened a coding agent",
            PaneKind::Viewer => "Opened a file viewer",
        };
        let mut outcome = self.said(words, Some(Labelled::new(Value::Integer(number), label)));
        outcome.undo = token_of(&pane_token(pane)).unwrap_or(Undoable::No);
        outcome
    }
}

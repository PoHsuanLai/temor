//! A plugin for temor's tests, built on `bayonet::testing`. It answers all four capabilities and
//! misbehaves on the wire when started with `--fault <name>` (bayonet's names).
//!
//! What it answers, by request:
//! - facts: `cluster = prod` (with a newline in the value) and one fact with no label;
//! - query: a batch of two rows for the generation asked, then complete; the text `stale` answers
//!   an older generation instead; the text `slow` sends batches until it is cancelled;
//! - activate: copy the row's id, except the row `evil`, which asks to open a file address;
//! - lens: a two by one picture; a command starting with `bad` draws a picture of the wrong size;
//! - adapt: working, then waiting for permission, until cancelled; in the folder `/finish` it
//!   reports done and finishes.

use std::process::ExitCode;
use std::time::Duration;

use bayonet::testing::{Ending, Fake, Fault, Step, fault_argument};
use temor_plugins::Cap;
use temor_plugins::message::{
    Activation, Batch, Fact, Icon, Item, Message, OpenTarget, Phase, Request,
};

const TICK: Duration = Duration::from_millis(20);

fn row(id: &str, title: &str, score: u32) -> Item {
    Item {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: String::new(),
        icon: Icon::None,
        score,
        actions: Vec::new(),
    }
}

fn main() -> ExitCode {
    let name = fault_argument(std::env::args().skip(1));
    let fault = name.as_deref().and_then(Fault::from_name);
    let fake = Fake::new(
        1,
        vec![
            Cap::BlockLens,
            Cap::OpenProvider,
            Cap::AgentAdapter,
            Cap::RowFacts,
        ],
        |protocol, provides| Message::Hello {
            protocol,
            provides: provides.to_vec(),
        },
        |request: &Request| matches!(request, Request::Cancel),
    )
    .with_fault(fault);
    fake.run(|request, conversation| {
        match request {
            Request::Facts { .. } => {
                conversation.say(
                    &Message::Facts(vec![
                        Fact {
                            label: "cluster".to_owned(),
                            value: "prod\n".to_owned(),
                        },
                        Fact {
                            label: String::new(),
                            value: "dropped".to_owned(),
                        },
                    ]),
                    &[],
                )?;
            }
            Request::Query { text, generation } if text == "slow" => loop {
                conversation.say(
                    &Message::Batch(Batch {
                        generation,
                        items: vec![row("slow", "slow", 1)],
                    }),
                    &[],
                )?;
                if conversation.cancelled() {
                    conversation.say(
                        &Message::Failed {
                            message: "cancelled".to_owned(),
                        },
                        &[],
                    )?;
                    break;
                }
                std::thread::sleep(TICK);
            },
            Request::Query { text, generation } => {
                let answers = match text.as_str() {
                    "stale" => generation.saturating_sub(1),
                    _ => generation,
                };
                conversation.say(
                    &Message::Batch(Batch {
                        generation: answers,
                        items: vec![
                            row("low", &format!("{text} low"), 100),
                            row("high", &format!("{text} high"), 900),
                        ],
                    }),
                    &[],
                )?;
                conversation.say(&Message::Complete, &[])?;
            }
            Request::Activate { item, .. } => {
                let activation = match item.id.as_str() {
                    "evil" => Activation::Open {
                        target: OpenTarget::Url("file:///etc/passwd".to_owned()),
                    },
                    other => Activation::Copy {
                        text: other.to_owned(),
                    },
                };
                conversation.say(&Message::Activation(activation), &[])?;
            }
            Request::Lens(block) if block.command.starts_with("bad") => {
                conversation.say(
                    &Message::Drawn {
                        width: 2,
                        height: 2,
                    },
                    &[1, 2, 3],
                )?;
            }
            Request::Lens(_) => {
                conversation.say(
                    &Message::Drawn {
                        width: 2,
                        height: 1,
                    },
                    &[255, 0, 0, 255, 0, 0, 255, 255],
                )?;
            }
            Request::Adapt { folder, .. } if folder == "/finish" => {
                for (phase, text) in [(Phase::Working, "thinking"), (Phase::Done, "all done")] {
                    conversation.say(
                        &Message::Event {
                            phase,
                            text: text.to_owned(),
                        },
                        &[],
                    )?;
                }
                conversation.say(&Message::Finished, &[])?;
            }
            Request::Adapt { .. } => {
                for (phase, text) in [
                    (Phase::Working, "thinking"),
                    (Phase::WaitingPermission, "wants to run ls"),
                ] {
                    conversation.say(
                        &Message::Event {
                            phase,
                            text: text.to_owned(),
                        },
                        &[],
                    )?;
                }
                while !conversation.cancelled() {
                    std::thread::sleep(TICK);
                }
                conversation.say(
                    &Message::Failed {
                        message: "cancelled".to_owned(),
                    },
                    &[],
                )?;
            }
            Request::Cancel => {}
        }
        Ok(Step::End(Ending::Finished))
    })
}

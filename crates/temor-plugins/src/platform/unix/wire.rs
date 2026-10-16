//! temor's protocol on bayonet's runner.

use bayonet::run::{Greeting, Protocol};

use crate::capability::Cap;
use crate::catalog::PROTOCOL;
use crate::message::{Message, Request};

/// The protocol marker.
#[derive(Debug, Clone, Copy)]
pub struct Wire;

impl Protocol for Wire {
    type Capability = Cap;
    type Request = Request;
    type Message = Message;
    const VERSION: u32 = PROTOCOL;

    fn greeting(message: &Message) -> Option<Greeting<'_, Cap>> {
        match message {
            Message::Hello { protocol, provides } => Some(Greeting {
                protocol: *protocol,
                provides,
            }),
            Message::Drawn { .. }
            | Message::Batch(_)
            | Message::Complete
            | Message::Activation(_)
            | Message::Event { .. }
            | Message::Facts(_)
            | Message::Finished
            | Message::Failed { .. } => None,
        }
    }
}

//! A `block.lens` plugin to try lenses with: under a finished `ls` block it draws a bar for each
//! line of the output, as long as the line, in the width temor asks for and about six rows tall.
//! The manifest beside it (`lens-demo.toml`) and the crate's README say how to install it.
//!
//! It speaks the plugin protocol on its standard input and output (bayonet's frames): it says
//! hello, reads one request, answers it, and ends.

use std::process::ExitCode;

use bayonet::wire::{WireError, read_frame, write_frame};
use temor_plugins::message::{LensBlock, Message, Request};
use temor_plugins::{Cap, PROTOCOL};

/// The most bars drawn; more lines share the last one's room.
const MOST_BARS: usize = 64;

/// The bars' colours, in turn.
const HUES: [[u8; 3]; 6] = [
    [97, 175, 239],
    [152, 195, 121],
    [229, 192, 123],
    [224, 108, 117],
    [198, 120, 221],
    [86, 182, 194],
];

const GROUND: [u8; 4] = [40, 44, 52, 255];
const RULE: [u8; 4] = [92, 99, 112, 255];

fn main() -> ExitCode {
    match serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lens_demo: {error}");
            ExitCode::FAILURE
        }
    }
}

fn say(message: &Message, payload: &[u8]) -> Result<(), WireError> {
    write_frame(&mut std::io::stdout().lock(), message, payload)
}

fn serve() -> Result<(), WireError> {
    say(
        &Message::Hello {
            protocol: PROTOCOL,
            provides: vec![Cap::BlockLens],
        },
        &[],
    )?;
    let request = read_frame::<_, Request>(&mut std::io::stdin().lock())?;
    match request.message {
        Request::Lens(block) => {
            let (width, height, rgba) = draw(&block);
            say(&Message::Drawn { width, height }, &rgba)
        }
        _ => say(
            &Message::Failed {
                message: "this plugin only draws blocks".to_owned(),
            },
            &[],
        ),
    }
}

/// The picture: a dark panel with a rule along its bottom and one bar per output line.
fn draw(block: &LensBlock) -> (u32, u32, Vec<u8>) {
    let width = block.width.clamp(16, 4096);
    let height = (width / 8).clamp(48, 220);
    let (w, h) = (width as usize, height as usize);
    let mut rgba = GROUND.repeat(w * h);
    let mut put = |x: usize, y: usize, colour: [u8; 4]| {
        if x < w && y < h {
            let at = (y * w + x) * 4;
            rgba[at..at + 4].copy_from_slice(&colour);
        }
    };
    let pad = 8.min(h / 4);
    let base = h - pad;
    for x in pad..w - pad {
        put(x, base, RULE);
    }
    let lengths: Vec<usize> = block
        .output
        .lines()
        .map(|line| line.trim_end().chars().count())
        .filter(|&n| n > 0)
        .take(MOST_BARS)
        .collect();
    let longest = lengths.iter().copied().max().unwrap_or(1).max(1);
    let room = (w - 2 * pad) / lengths.len().max(1);
    let bar = (room * 3 / 4).max(1);
    let tallest = base.saturating_sub(pad);
    for (i, &len) in lengths.iter().enumerate() {
        let [r, g, b] = HUES[i % HUES.len()];
        let tall = (tallest * len / longest).max(2);
        let left = pad + i * room + (room - bar) / 2;
        for y in base - tall..base {
            for x in left..left + bar {
                put(x, y, [r, g, b, 255]);
            }
        }
    }
    (width, height, rgba)
}

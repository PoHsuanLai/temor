//! The sending side of the kitty protocol: the pure `SendSession` that offers files and streams
//! them, `serve` that answers a `receive` request from the shelf, and the small file readers
//! the edges use.

mod io;
mod model;
mod serve;
mod step;

pub use io::{describe, read_chunk};
pub use model::{ChunkEnd, SendEffect, SendEnd, SendFile, SendInput, SendSession};
pub use serve::{Offered, Served, serve};

//! Finding the files a terminal's text names (PLAN §10): the path under a click, and the paths a
//! command printed. Pure: the caller says which paths are files, so the file system is the edge's.

mod output;
mod token;

#[cfg(test)]
mod tests;

pub use output::{MAX_PATHS, output_paths, resolve};
pub use token::{candidates, token_at};

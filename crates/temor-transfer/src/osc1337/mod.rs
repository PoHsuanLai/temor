//! iTerm2's OSC 1337 file transfers: `File=` (one sequence) and `MultipartFile=` / `FilePart=` /
//! `FileEnd` (a file in pieces). temor-vt pre-scans `File=` before libghostty and hands over the
//! payload; this module decodes it.

mod assemble;
mod model;
mod parse;

pub use assemble::Osc1337Reader;
pub use model::{
    Aspect, Completeness, Delivery, Dimension, FileHeader, FileOffer, InlineImage, MimeType,
    ReceivedFile,
};

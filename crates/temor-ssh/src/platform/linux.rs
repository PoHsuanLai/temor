//! Linux: a non-blocking open of a regular file. (The private runtime directory and the socket
//! path limit are temor-shell's.)

use std::fs::File;
use std::io::{self, ErrorKind};

use rustix::fs::{Mode, OFlags};
use std::path::Path;

/// Opens `path` for reading without blocking, and only if it is a regular file. A FIFO, a
/// device such as `/dev/zero`, or a directory is refused here, after a non-blocking open that
/// cannot hang, so a config `Include` can never stall or stream forever.
pub fn open_regular(path: &Path) -> io::Result<File> {
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).map_err(io::Error::from)?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(ErrorKind::InvalidData, "not a regular file"));
    }
    Ok(file)
}

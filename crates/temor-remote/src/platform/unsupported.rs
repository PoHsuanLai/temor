//! Every other system: nothing here yet, and each call says so.
// TODO(platform:macos): openat(O_NOFOLLOW) walks for the file service, `libproc` for ports.
// TODO(platform:windows): the helper targets Linux hosts only; Windows hosts stay on Tier A.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use temor_wire::message::ListeningPort;

use super::RawStat;
use crate::error::Error;

fn no(feature: &'static str) -> Error {
    Error::Unsupported { feature }
}

/// A confined folder (unsupported here).
#[derive(Debug)]
pub struct Beneath;

impl Beneath {
    pub fn open(_root: &Path) -> Result<Self, Error> {
        Err(no("confined file access"))
    }

    pub fn stat(&self, _rel: &[String], _shown: &str) -> Result<RawStat, Error> {
        Err(no("confined file access"))
    }

    pub fn read_dir(&self, _rel: &[String], _shown: &str) -> Result<Vec<(String, RawStat)>, Error> {
        Err(no("confined file access"))
    }

    pub fn open_file(&self, _rel: &[String], _shown: &str) -> Result<(File, RawStat), Error> {
        Err(no("confined file access"))
    }

    pub fn replace(
        &self,
        _rel: &[String],
        _bytes: &[u8],
        _nonce: u64,
        _shown: &str,
    ) -> Result<(), Error> {
        Err(no("confined file access"))
    }

    pub fn create_new(&self, _rel: &[String], _bytes: &[u8], _shown: &str) -> Result<(), Error> {
        Err(no("confined file access"))
    }
}

pub fn listening_ports(_proc: &Path, _roots: &[u32]) -> Result<Vec<ListeningPort>, Error> {
    Err(no("port detection"))
}

pub fn ensure_private_dir(_path: &Path) -> Result<(), Error> {
    Err(no("a private folder"))
}

/// One end of a connection (unsupported here).
#[derive(Debug)]
pub struct Half;

impl Half {
    pub fn duplicate(&self) -> Result<Self, Error> {
        Err(no("the local socket"))
    }

    pub fn shutdown_write(&self) {}
}

impl Read for Half {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Write for Half {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The listening socket (unsupported here).
#[derive(Debug)]
pub struct Listener;

impl Listener {
    pub fn bind(_path: &Path) -> Result<Self, Error> {
        Err(no("the local socket"))
    }

    pub fn accept(&self) -> Result<Half, Error> {
        Err(no("the local socket"))
    }
}

pub fn connect(_path: &Path) -> Result<Half, Error> {
    Err(no("the local socket"))
}

pub fn spawn_detached(_program: &Path, _args: &[&str]) -> Result<(), Error> {
    Err(no("starting the daemon"))
}

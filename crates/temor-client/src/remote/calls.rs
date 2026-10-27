//! The calls on a helper's client that the window makes, in one place; the rest of the window
//! never names these types.

use std::time::Duration;

use temor_remote::{Attached, Client, Error};
use temor_wire::ids::SessionId;
use temor_wire::message::{EnvVar, ListeningPort, OpenSession, Palette, RemotePath};
use temor_wire::units::{CellPx, Cols, Pixels, Rows, Size};

/// One cell in the pixels the helper wants, from the pane's cell, `width` by `height` device pixels. A side that
/// does not fit 16 bits is held at the most; a cell with no width or height (a pane not laid out
/// yet) is the helper's own default.
pub fn wire_cell(width: u32, height: u32) -> CellPx {
    let side = |px: u32| u16::try_from(px).unwrap_or(u16::MAX);
    match (side(width), side(height)) {
        (0, _) | (_, 0) => CellPx::DEFAULT,
        (width, height) => CellPx {
            width: Pixels(width),
            height: Pixels(height),
        },
    }
}

/// The size of a terminal of `cols` by `rows` cells.
pub fn grid_size(cols: u16, rows: u16) -> Size {
    Size {
        cols: Cols(cols),
        rows: Rows(rows),
    }
}

/// Opens a session running the account's login shell in `cwd`, with `env` as its whole
/// environment, at `size` cells of `cell` pixels, answering colour questions with `palette`.
pub fn open(
    client: &Client,
    (cwd, env, shell): (&str, &[(String, String)], Option<&str>),
    (size, cell): (Size, CellPx),
    palette: Palette,
) -> Result<SessionId, Error> {
    client.open_session(OpenSession {
        shell: shell.map(|shell| RemotePath(shell.to_owned())),
        cwd: RemotePath(cwd.to_owned()),
        env: env
            .iter()
            .map(|(name, value)| EnvVar {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        size,
        cell,
        palette: Some(palette),
    })
}

pub fn attach(client: &Client, session: SessionId) -> Result<Attached, Error> {
    client.attach(session)
}

pub fn resize(
    client: &Client,
    session: SessionId,
    (size, cell): (Size, CellPx),
) -> Result<(), Error> {
    client.resize(session, size, cell)
}

pub fn set_palette(client: &Client, session: SessionId, palette: Palette) -> Result<(), Error> {
    client.set_palette(session, palette)
}

/// Asks the helper whether it is there, waiting at most `within`.
pub fn ping(client: &Client, within: Duration) -> Result<(), Error> {
    client.ping(within)
}

/// The sessions the host holds.
pub fn sessions(client: &Client) -> Result<Vec<SessionId>, Error> {
    Ok(client
        .list_sessions()?
        .into_iter()
        .map(|info| info.session)
        .collect())
}

pub fn input(client: &Client, session: SessionId, bytes: &[u8]) -> Result<(), Error> {
    client.send_input(session, bytes)
}

pub fn close(client: &Client, session: SessionId) -> Result<(), Error> {
    client.close_session(session)
}

pub fn ports(client: &Client) -> Result<Vec<ListeningPort>, Error> {
    client.list_ports()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_goes_to_the_helper_in_pixels_it_can_hold() {
        let cases = [
            ((9, 18), (9, 18)),
            ((18, 36), (18, 36)),
            ((70_000, 20), (u16::MAX, 20)),
            ((0, 18), (8, 16)),
            ((9, 0), (8, 16)),
        ];
        for ((w, h), (want_w, want_h)) in cases {
            let got = wire_cell(w, h);
            assert_eq!(
                (got.width, got.height),
                (Pixels(want_w), Pixels(want_h)),
                "{w}x{h}"
            );
        }
    }
}

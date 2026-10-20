//! A long folder in its chip keeps its end. The chip has only the room the row leaves it, and
//! Blitz has no `text-overflow: ellipsis`, so the folder is cut here, by counting characters
//! against that room: the start goes, with a leading "…", and the last folders stay, since the
//! end is what tells folders apart.

use temor_core::facts::{CHIP_FRAME, CHIP_GAP};

use super::fact_chips::{ChipKind, ChipText};

/// What replaces the cut start.
const ELLIPSIS: char = '\u{2026}';

/// The fewest characters the folder chip is cut down to, so it never shrinks to nothing.
const LEAST_ROOM: usize = 4;

/// The row the chips stand on, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowRoom {
    /// From where the first chip starts to where the row ends.
    pub width: f32,
    /// One cell of the grid.
    pub cell_width: f32,
}

/// `folder` cut to at most `room` characters. Whole folders from the end are kept when they
/// fit (`…/app/src`); when even the last one is too long its end is kept (`…ong-name`).
pub fn fit_folder(folder: &str, room: usize) -> String {
    if folder.chars().count() <= room {
        return folder.to_owned();
    }
    let keep = room.saturating_sub(1);
    // The earliest slash from which the rest fits.
    let from_slash = folder
        .match_indices('/')
        .map(|(at, _)| &folder[at..])
        .find(|rest| rest.chars().count() <= keep && rest.len() > 1);
    let tail = match from_slash {
        Some(rest) => rest,
        None => {
            let skip = folder.chars().count().saturating_sub(keep);
            let at = folder
                .char_indices()
                .nth(skip)
                .map_or(folder.len(), |(at, _)| at);
            &folder[at..]
        }
    };
    format!("{ELLIPSIS}{tail}")
}

/// How many characters the folder chip's words may have, when the other chips (their word
/// counts, a picture counted as it is elsewhere) share the row with it.
pub fn folder_room(row: RowRoom, others: &[usize]) -> usize {
    if row.cell_width <= 0.0 {
        return usize::MAX;
    }
    let cells = row.width / row.cell_width;
    let beside: f32 = others.iter().map(|&n| n as f32 + CHIP_FRAME + CHIP_GAP).sum();
    let room = (cells - beside - CHIP_FRAME).floor();
    (room.max(0.0) as usize).max(LEAST_ROOM)
}

/// Cuts the start of the folder chip among `chips`, if there is one and it is too long for its
/// row.
pub fn fit_folder_chip(chips: &mut [ChipText], row: RowRoom) {
    let words = |chip: &ChipText| {
        chip.text().chars().count() + CHIP_FRAME as usize * usize::from(chip.icon.is_some())
    };
    let others: Vec<usize> = chips
        .iter()
        .filter(|chip| chip.kind != ChipKind::Folder)
        .map(words)
        .collect();
    let room = folder_room(row, &others);
    for chip in chips
        .iter_mut()
        .filter(|chip| chip.kind == ChipKind::Folder)
    {
        for part in &mut chip.parts {
            part.text = fit_folder(&part.text, room);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::fact_chips::{ChipIcon, ChipPart, Tone};

    #[test]
    fn a_folder_too_long_for_its_room_loses_its_start_and_keeps_its_last_folders() {
        // (why, folder, room, want)
        let table = [
            ("it fits with room to spare", "~/work/app", 20, "~/work/app"),
            ("it fits exactly", "~/work/app", 10, "~/work/app"),
            (
                "one folder too many",
                "~/work/app/src",
                10,
                "\u{2026}/app/src",
            ),
            (
                "as many as fit",
                "~/code/work/app/src",
                15,
                "\u{2026}/work/app/src",
            ),
            (
                "the last folder alone is too long",
                "~/work/long-name",
                8,
                "\u{2026}ng-name",
            ),
            (
                "room for one character after the dots",
                "~/work/app",
                2,
                "\u{2026}p",
            ),
            ("no room at all", "~/work/app", 0, "\u{2026}"),
            ("a root folder", "/srv/data/logs", 9, "\u{2026}/logs"),
            (
                "wide letters count as one each",
                "~/\u{4e2d}\u{6587}/\u{4e2d}\u{6587}\u{6587}",
                6,
                "\u{2026}/\u{4e2d}\u{6587}\u{6587}",
            ),
        ];
        for (why, folder, room, want) in table {
            let got = fit_folder(folder, room);
            assert_eq!(got, want, "{why}");
            assert!(
                got.chars().count() <= room.max(1),
                "{why}: {got} is within {room}"
            );
        }
    }

    #[test]
    fn the_folder_gets_the_row_minus_the_chips_beside_it() {
        // (why, width, cell width, other chips' words, want)
        let table = [
            ("alone: the row less its picture", 400.0, 10.0, vec![], 38),
            (
                "a branch beside it takes its words, picture and a gap",
                400.0,
                10.0,
                vec![6],
                29,
            ),
            ("two beside it", 400.0, 10.0, vec![6, 14], 12),
            (
                "squeezed: never below the least",
                100.0,
                10.0,
                vec![6, 14],
                LEAST_ROOM,
            ),
            (
                "a cell of no width has no limit",
                400.0,
                0.0,
                vec![6],
                usize::MAX,
            ),
        ];
        for (why, width, cell_width, others, want) in table {
            let got = folder_room(RowRoom { width, cell_width }, &others);
            assert_eq!(got, want, "{why}");
        }
    }

    fn chip(kind: ChipKind, icon: Option<ChipIcon>, text: &str) -> ChipText {
        ChipText {
            kind,
            icon,
            parts: vec![ChipPart {
                text: text.to_owned(),
                tone: Tone::Strong,
            }],
        }
    }

    #[test]
    fn only_the_folder_chip_is_cut() {
        let mut chips = vec![
            chip(
                ChipKind::Folder,
                Some(ChipIcon::Folder),
                "~/work/app/src/deep/er",
            ),
            chip(ChipKind::Branch, Some(ChipIcon::Branch), "main"),
        ];
        // 30 cells: the branch takes 4 + 2 + 1, the folder's picture 2: 19 characters remain.
        fit_folder_chip(
            &mut chips,
            RowRoom {
                width: 300.0,
                cell_width: 10.0,
            },
        );
        assert_eq!(chips[0].text(), "\u{2026}/app/src/deep/er");
        assert_eq!(chips[1].text(), "main");
    }
}

//! The control bytes the scanners share, read off libghostty's parse table
//! (`src/terminal/parse_table.zig`, `osc_string` row).

pub(super) const BEL: u8 = 0x07;
pub(super) const CAN: u8 = 0x18;
pub(super) const SUB: u8 = 0x1A;
pub(super) const ESC: u8 = 0x1B;

/// Whether `b` leaves an OSC string: BEL ends it, CAN and SUB abort it, and ESC leaves it for
/// whatever it starts (usually the `\` of ST). The C1 bytes 0x9C and 0x9D are not in this list:
/// the `osc_string` row makes every byte from 0x20 up payload, over the parse table's "anywhere"
/// C1 rules, because libghostty is UTF-8 only (a raw 0x9C is a continuation byte).
pub(super) fn ends_osc(b: u8) -> bool {
    matches!(b, BEL | CAN | SUB | ESC)
}

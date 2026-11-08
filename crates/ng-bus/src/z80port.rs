//! Z80 pin server state for SoundBoard tick loop.
//!
//! Stores the last output pins, pending read data, and write completion flag
//! so that a single T-state can be served per call to `SoundBoard::tick`.

use puce_z80::Pins;

/// Pin-server state carried between ticks of one Z80 instruction sequence.
#[derive(Default)]
pub struct Z80Port {
    /// Output pins from the previous tick (used to detect read/write requests).
    pub last: Pins,
    /// Data requested by a previous MREQ|RD or IORQ|RD; None if not pending.
    pub read_data: Option<u8>,
    /// True once the current write cycle has been performed (prevents double-write on WAIT).
    pub write_done: bool,
}

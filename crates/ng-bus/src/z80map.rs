//! Z80 memory map helpers: banking offset calculation.
//!
//! The Neo Geo Z80 has four banked windows that provide access to the full
//! 512 Kio of M1 ROM. This module contains the address translation logic.

/// Compute the offset into M1 ROM for a banked window read.
///
/// # Arguments
/// * `m1_len` - Total length of the M1 ROM in bytes
/// * `bank_num` - Bank number (0-255) selected via port $08-$0B
/// * `window_idx` - Window index: 0=$F000(2K), 1=$E000(4K), 2=$C000(8K), 3=$8000(16K)
/// * `addr` - Z80 address within the window
pub fn banked_offset(m1_len: usize, bank_num: u8, window_idx: usize, addr: u16) -> usize {
    let bases = [0xF000u16, 0xE000, 0xC000, 0x8000];
    let base = bases[window_idx];

    // mask = (m1.len() - 0x10000 - 1) & 0x3FFFF
    let mask = ((m1_len as u32).wrapping_sub(0x10001)) & 0x3FFFF;
    let bank_addr = (bank_num as u32) << (11 + window_idx);
    let offset = 0x10000u32 + ((bank_addr & mask) | ((addr - base) as u32));
    offset as usize
}

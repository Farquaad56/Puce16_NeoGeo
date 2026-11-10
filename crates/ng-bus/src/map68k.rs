//! 68000 bus memory map for Neo Geo MVS/AES.
//!
//! Implements the complete address decoding as specified in docs/spec/01-memory-map-registers.md.

use crate::device::{CartPort, Device, NullDevice};
use crate::pbank;
use ng_cart::Model;

/// The 68000 system bus with full memory mapping.
pub struct Bus68k {
    /// Machine model (MVS arcade or AES console).
    pub model: Model,
    /// Work RAM: $100000-$10FFFF (mirrored to $110000-$1FFFFF)
    pub work_ram: Box<[u8; 0x10000]>,
    /// BIOS ROM: 128 KiB big-endian words ($C00000, mirrored)
    pub bios: Vec<u8>,
    /// Cart P-ROM region (may be empty for some games)
    pub cart_p: Vec<u8>,
    /// Latch Q1 from REG_SWPROM: use cart vectors instead of BIOS
    pub vectors_from_cart: bool,
    /// Bank select offset into cart_p for the $200000 window (T306)
    pub pbank_base: u32,
    /// Value returned on unmapped reads (open bus)
    pub open_bus: u16,
    /// I/O devices at $300000-$3FFFFF (excluding LSPC)
    pub io: Box<dyn Device>,
    /// LSPC video registers at $3C0000-$3C000F
    pub lspc: Box<dyn Device>,
    /// Palette RAM at $400000-$7FFFFF
    pub palette: Box<dyn Device>,
    /// Backup RAM at $D00000-$DFFFFF (MVS only)
    pub backup: Box<dyn Device>,
    /// Memory card at $800000-$BFFFFF
    pub memcard: Box<dyn Device>,
    /// Cart protection chip on the $200000 window (optional, P7)
    pub port: Option<Box<dyn CartPort>>,
}

impl Bus68k {
    /// Create a new bus with null devices for all peripherals.
    pub fn new(bios: Vec<u8>, cart_p: Vec<u8>) -> Self {
        let pbank_base = pbank::initial_pbank_base(cart_p.len());
        Self {
            model: Model::Mvs,
            work_ram: Box::new([0u8; 0x10000]),
            bios,
            cart_p,
            vectors_from_cart: false,
            pbank_base,
            open_bus: 0xFFFF,
            io: Box::new(NullDevice),
            lspc: Box::new(NullDevice),
            palette: Box::new(NullDevice),
            backup: Box::new(NullDevice),
            memcard: Box::new(NullDevice),
            port: None,
        }
    }

    /// Read a 16-bit word from the bus at address `addr`.
    pub fn read16(&mut self, addr: u32) -> u16 {
        let a = addr & 0xFFFFFE; // Always aligned to even boundary
        match a >> 20 {
            0x0 => {
                if a < 0x80 {
                    // Vector area: BIOS or cart depending on latch
                    if self.vectors_from_cart {
                        read_word_be(&self.cart_p, a)
                    } else {
                        read_word_be(&self.bios, a)
                    }
                } else {
                    // Cart P-ROM fixed bank
                    let end = a + 2;
                    if end <= self.cart_p.len() as u32 && end <= 0x100000 {
                        read_word_be(&self.cart_p, a)
                    } else {
                        self.open_bus
                    }
                }
            }
            0x1 => {
                // Work RAM (mirrored $110000-$1FFFFF via >>20 check below)
                read_word_be(&self.work_ram[..], a & 0xFFFF)
            }
            _ if (a >> 20) == 1 => {
                // RAM mirrors $110000-$1FFFFF
                read_word_be(&self.work_ram[..], a & 0xFFFF)
            }
            0x2 => {
                // Cart P-ROM window with bank switching and port protection
                if let Some(ref mut port) = self.port {
                    if let Some(v) = port.read(a, &self.cart_p, &self.work_ram[..]) {
                        return v;
                    }
                }
                let offset = self.pbank_base + (a & 0xFFFFF);
                let end = offset + 2;
                if end <= self.cart_p.len() as u32 {
                    read_word_be(&self.cart_p, offset)
                } else {
                    self.open_bus
                }
            }
            0x3 => {
                // I/O and LSPC registers
                match (a >> 16) & 0xFE {
                    0x3C => self.lspc.read(a),
                    0x30 | 0x32 | 0x34 | 0x38 | 0x3A => self.io.read(a),
                    _ => self.open_bus,
                }
            }
            n if (4..=7).contains(&n) => {
                // Palette RAM
                self.palette.read(a)
            }
            n if (8..=0xB).contains(&n) => {
                // Memory card
                self.memcard.read(a)
            }
            0xC => {
                // BIOS ROM (mirrored $C20000-$CFFFFF via & 0x1FFFF)
                read_word_be(&self.bios, a & 0x1FFFF)
            }
            0xD => {
                // Backup RAM (MVS only; AES has no backup RAM)
                if self.model == Model::Aes {
                    self.open_bus
                } else {
                    self.backup.read(a)
                }
            }
            _ => self.open_bus,
        }
    }

    /// Write a 16-bit word to the bus at address `addr`.
    pub fn write16(&mut self, addr: u32, data: u16, upper: bool, lower: bool) {
        let a = addr & 0xFFFFFE; // Always aligned
        match a >> 20 {
            0x0 => {
                // ROM area: writes ignored (except vector latch via system regs at $3A00xx)
            }
            0x1 => {
                // Work RAM write with byte enable flags
                let offset = a & 0xFFFF;
                if upper {
                    self.work_ram[offset as usize] = (data >> 8) as u8;
                }
                if lower {
                    self.work_ram[(offset + 1) as usize] = data as u8;
                }
            }
            _ if (a >> 20) == 1 => {
                // RAM mirrors: same write logic
                let offset = a & 0xFFFF;
                if upper {
                    self.work_ram[offset as usize] = (data >> 8) as u8;
                }
                if lower {
                    self.work_ram[(offset + 1) as usize] = data as u8;
                }
            }
            0x2 => {
                // Fenêtre P-ROM : la puce de protection (si présente) a priorité, sinon sélection
                // de banque standard en $2FFFF0-$2FFFFE.
                let mut consumed = false;
                if let Some(ref mut port) = self.port {
                    if port.write(a, data, upper, lower, &self.cart_p, &self.work_ram[..]) {
                        consumed = true;
                        if let Some(bank) = port.take_bank() {
                            self.pbank_base = bank;
                        }
                    }
                }
                if !consumed && (0x2FFFF0..=0x2FFFFE).contains(&a) {
                    pbank::banksel(&mut self.pbank_base, self.cart_p.len(), data);
                }
            }
            0x3 => {
                // I/O and LSPC register writes
                match (a >> 16) & 0xFE {
                    0x3C => self.lspc.write(a, data, upper, lower),
                    0x30 | 0x32 | 0x34 | 0x38 | 0x3A => {
                        // `Io` décode l'octet par la parité de l'adresse (pair = octet bas du mot,
                        // impair = octet haut) ; le 68000 donne une adresse de mot et choisit la voie
                        // par uds/lds (pair -> D8-D15, impair -> D0-D7) : conversion ici.
                        match (upper, lower) {
                            (false, true) => self.io.write(a | 1, data << 8, upper, lower),
                            (true, false) => self.io.write(a, data >> 8, upper, lower),
                            _ => self.io.write(a, data, upper, lower),
                        }
                    }
                    _ => {} // Unmapped I/O: ignored
                }
            }
            n if (4..=7).contains(&n) => {
                // Palette RAM write
                self.palette.write(a, data, upper, lower);
            }
            n if (8..=0xB).contains(&n) => {
                // Memory card write
                self.memcard.write(a, data, upper, lower);
            }
            0xC => {
                // BIOS ROM: writes ignored
            }
            0xD if self.model == Model::Mvs => {
                // Backup RAM write (MVS only)
                self.backup.write(a, data, upper, lower);
            }
            _ => {} // Unmapped: ignored
        }
    }
}

/// Read a big-endian word from a byte slice at the given offset.
fn read_word_be(data: &[u8], offset: u32) -> u16 {
    let o = offset as usize;
    ((data[o] as u16) << 8) | (data[o + 1] as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bus() -> Bus68k {
        let bios = vec![0u8; 0x20000];
        let cart_p = vec![0u8; 0x100000];
        Bus68k::new(bios, cart_p)
    }

    #[test]
    fn ram_write_read() {
        let mut bus = make_bus();
        bus.write16(0x100000, 0x1234, true, true);
        assert_eq!(bus.read16(0x100000), 0x1234);
    }

    #[test]
    fn ram_mirror_read() {
        let mut bus = make_bus();
        bus.write16(0x100000, 0x1234, true, true);
        assert_eq!(bus.read16(0x110000), 0x1234); // Mirror
        assert_eq!(bus.read16(0x1F0000), 0x1234); // Far mirror
    }

    #[test]
    fn ram_byte_write_upper_only() {
        let mut bus = make_bus();
        // Write a known value at offset 2-3 first
        bus.write16(0x100002, 0xFFFF, true, true);
        // Now write only upper byte
        bus.write16(0x100002, 0xAB00, true, false);
        assert_eq!(bus.work_ram[2], 0xAB);
        assert_eq!(bus.work_ram[3], 0xFF); // Lower unchanged
    }

    #[test]
    fn bios_read() {
        let mut bus = make_bus();
        bus.bios[0] = 0x00;
        bus.bios[1] = 0x10;
        bus.bios[2] = 0x00;
        bus.bios[3] = 0x00;
        let result = bus.read16(0xC00000);
        println!("bios read at 0xC00000: {:X} (expected 0x0010)", result);
        assert_eq!(result, 0x0010);
    }

    #[test]
    fn bios_write_ignored() {
        let mut bus = make_bus();
        bus.bios[0] = 0x00;
        bus.bios[1] = 0x10;
        bus.write16(0xC00000, 0xFFFF, true, true);
        assert_eq!(bus.read16(0xC00000), 0x0010); // Unchanged
    }

    #[test]
    fn vectors_from_bios_default() {
        let mut bus = make_bus();
        bus.bios[0] = 0x11;
        bus.bios[1] = 0x22;
        bus.cart_p[0] = 0xAA;
        bus.cart_p[1] = 0xBB;
        assert_eq!(bus.read16(0), 0x1122); // BIOS vectors by default
    }

    #[test]
    fn vectors_from_cart_after_latch() {
        let mut bus = make_bus();
        bus.bios[0] = 0x11;
        bus.bios[1] = 0x22;
        bus.cart_p[0] = 0xAA;
        bus.cart_p[1] = 0xBB;
        bus.vectors_from_cart = true;
        assert_eq!(bus.read16(0), 0xAABB); // Cart vectors after latch
    }

    #[test]
    fn cart_fixed_bank_read() {
        let mut bus = make_bus();
        bus.cart_p[0x80] = 0xDE;
        bus.cart_p[0x81] = 0xAD;
        assert_eq!(bus.read16(0x80), 0xDEAD); // Cart P-ROM at $80+
    }

    #[test]
    fn cart_window_bank_select() {
        let mut bus = make_bus();
        bus.cart_p.resize(0x300000, 0);
        bus.cart_p[0x100000] = 0xCA;
        bus.cart_p[0x100001] = 0xFE;
        bus.pbank_base = 0x100000;
        assert_eq!(bus.read16(0x200000), 0xCAFE); // Window reads from bank offset
    }

    #[test]
    fn unmapped_read_open_bus() {
        let mut bus = make_bus();
        bus.open_bus = 0xBEEF;
        assert_eq!(bus.read16(0xE00000), 0xBEEF); // Unmapped area
    }

    #[test]
    fn io_device_read() {
        let mut bus = make_bus();
        // Write to work RAM, then read via I/O (should get open_bus since NullDevice)
        assert_eq!(bus.read16(0x300000), 0xFFFF); // NullDevice returns FFFF
    }

    #[test]
    fn palette_read() {
        let mut bus = make_bus();
        assert_eq!(bus.read16(0x400000), 0xFFFF); // NullDevice
    }

    #[test]
    fn memcard_read() {
        let mut bus = make_bus();
        assert_eq!(bus.read16(0x800000), 0xFFFF); // NullDevice
    }

    #[test]
    fn backup_ram_read() {
        let mut bus = make_bus();
        assert_eq!(bus.read16(0xD00000), 0xFFFF); // NullDevice
    }
}

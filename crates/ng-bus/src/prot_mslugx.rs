//! Metal Slug X protection chip emulation ($2FFFE0-$2FFFEF).
//! Port of MAME prot_mslugx.cpp (lines 35-87).

use crate::device::CartPort;

/// Metal Slug X protection chip state.
#[derive(Default)]
pub struct MslugX {
    command: u16,
    counter: u16,
    accesses: u64,
}

impl MslugX {
    pub fn new() -> Self {
        Self {
            command: 0,
            counter: 0,
            accesses: 0,
        }
    }
}

impl CartPort for MslugX {
    fn access_count(&self) -> u64 {
        self.accesses
    }

    fn read(&mut self, a: u32, p: &[u8], wram: &[u8]) -> Option<u16> {
        // Only active in $2FFFE0-$2FFFEF range
        if !(0x2FFFE0..=0x2FFFEF).contains(&a) {
            return None;
        }
        self.accesses += 1;

        match self.command {
            0x0001 => {
                // Read bit from P-ROM based on counter
                let byte_addr = 0xDEDD2 + (((self.counter >> 3) & 0xFFF) as usize);
                let shift = (!self.counter & 7) as u8;
                let val = p[byte_addr];
                let res = ((val >> shift) & 1) as u16;
                self.counter += 1;
                Some(res)
            }
            0x0FFF => {
                // Read bit from P-ROM based on select value in RAM
                let ram_offset = 0xF00A;
                let word_val = ((wram[ram_offset] as u16) << 8) | (wram[ram_offset + 1] as u16);
                let select = (word_val as i32) - 1;
                let byte_addr = 0xDEDD2 + (((select >> 3) & 0xFFF) as usize);
                let shift = (!select & 7) as u8;
                let val = p[byte_addr];
                Some(((val >> shift) & 1) as u16)
            }
            _ => {
                // Unknown command: return 0
                Some(0)
            }
        }
    }

    fn write(
        &mut self,
        a: u32,
        d: u16,
        _upper: bool,
        _lower: bool,
        _p: &[u8],
        _wram: &[u8],
    ) -> bool {
        // Only active in $2FFFE0-$2FFFEF range
        if !(0x2FFFE0..=0x2FFFEF).contains(&a) {
            return false;
        }
        self.accesses += 1;

        let offset = (a - 0x2FFFE0) >> 1;
        match offset {
            0 => {
                // Start new read sequence
                self.command = 0;
            }
            1 | 2 => {
                // Command bits (pulsed with data then 0)
                self.command |= d;
            }
            3 => {
                // Finished? (no action)
            }
            5 => {
                // Init: reset counter and command
                self.counter = 0;
                self.command = 0;
            }
            _ => {}
        }

        true // Write consumed
    }

    fn reset(&mut self) {
        self.command = 0;
        self.counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x100000];
        // Set test byte at 0xDEDD2: 0b0000_0101 (bits 2 and 0 set)
        rom[0xDEDD2] = 0b0000_0101;
        rom
    }

    fn make_test_wram() -> Vec<u8> {
        vec![0u8; 0x10000]
    }

    #[test]
    fn write_command_sequence() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Write offset 0: command = 0
        assert!(prot.write(0x2FFFE0, 0, true, true, &rom, &wram));
        assert_eq!(prot.command, 0);

        // Write offset 1 with data=1: command |= 1
        assert!(prot.write(0x2FFFE2, 1, true, true, &rom, &wram));
        assert_eq!(prot.command, 1);

        // Write offset 2 with data=8: command |= 8
        assert!(prot.write(0x2FFFE4, 8, true, true, &rom, &wram));
        assert_eq!(prot.command, 9);
    }

    #[test]
    fn write_offset5_resets() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Set some state
        prot.write(0x2FFFE2, 1, true, true, &rom, &wram);
        assert_eq!(prot.command, 1);

        // Write offset 5 (address 0x2FFFEA): reset counter and command
        assert!(prot.write(0x2FFFEA, 0, true, true, &rom, &wram));
        assert_eq!(prot.counter, 0);
        assert_eq!(prot.command, 0);
    }

    #[test]
    fn read_command_0x0001_bit_sequence() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Set command to 0x0001 via writes
        prot.write(0x2FFFE0, 0, true, true, &rom, &wram);
        prot.write(0x2FFFE2, 1, true, true, &rom, &wram);

        // p[0xDEDD2] = 0b0000_0101
        // counter=0: shift=!0&7=7, (byte>>7)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=1: shift=!1&7=6, (byte>>6)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=2: shift=!2&7=5, (byte>>5)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=3: shift=!3&7=4, (byte>>4)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=4: shift=!4&7=3, (byte>>3)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=5: shift=!5&7=2, (byte>>2)&1 = 1 (bit 2 is set)
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(1));
        // counter=6: shift=!6&7=1, (byte>>1)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
        // counter=7: shift=!7&7=0, (byte>>0)&1 = 1 (bit 0 is set)
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(1));
    }

    #[test]
    fn read_command_0x0fff_select() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let mut wram = make_test_wram();

        // Set command to 0x0FFF via writes (offsets 1 and 2)
        prot.write(0x2FFFE0, 0, true, true, &rom, &wram);
        prot.write(0x2FFFE2, 0xFF, true, true, &rom, &wram);
        prot.write(0x2FFFE4, 0xF0, true, true, &rom, &wram);

        // Set wram[0xF00A..] = 0x0001 (select = 0)
        wram[0xF00A] = 0x00;
        wram[0xF00B] = 0x01;

        // select=0: shift=!0&7=7, (byte>>7)&1 = 0
        assert_eq!(prot.read(0x2FFFE0, &rom, &wram), Some(0));
    }

    #[test]
    fn read_out_of_range_returns_none() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Address below range (0x2FFFDF < 0x2FFFE0)
        assert_eq!(prot.read(0x2FFFDF, &rom, &wram), None);
        // Address above range (0x300000 > 0x2FFFEF)
        assert_eq!(prot.read(0x300000, &rom, &wram), None);
    }

    #[test]
    fn write_out_of_range_returns_false() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Address below range (0x2FFFDF < 0x2FFFE0)
        assert!(!prot.write(0x2FFFDF, 0, true, true, &rom, &wram));
        // Address above range (0x300000 > 0x2FFFEF)
        assert!(!prot.write(0x300000, 0, true, true, &rom, &wram));
    }

    #[test]
    fn reset_clears_state() {
        let mut prot = MslugX::new();
        let rom = make_test_rom();
        let wram = make_test_wram();

        // Set some state
        prot.write(0x2FFFE2, 1, true, true, &rom, &wram);
        assert_eq!(prot.command, 1);

        prot.reset();
        assert_eq!(prot.command, 0);
        assert_eq!(prot.counter, 0);
    }
}

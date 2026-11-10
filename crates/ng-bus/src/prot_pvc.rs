//! PVC Metal Slug 5 protection chip emulation ($2FE000-$2FFFFF).
//! Port of MAME prot_pvc.cpp (lines 28-98: protection_r/w, unpack/pack color, get_bank_base).

use crate::device::CartPort;

/// Combine byte writes into a 16-bit value (MAME COMBINE_DATA macro).
fn combine_data(old: u16, new: u16, upper: bool, lower: bool) -> u16 {
    if upper && lower {
        return new;
    }
    let mut result = old;
    if upper {
        result = (result & 0x00FF) | (new & 0xFF00);
    }
    if lower {
        result = (result & 0xFF00) | (new & 0x00FF);
    }
    result
}

/// PVC protection chip state.
pub struct Pvc {
    ram: [u16; 0x1000],
    bank: Option<u32>,
}

impl Default for Pvc {
    fn default() -> Self {
        Self::new()
    }
}

impl Pvc {
    pub fn new() -> Self {
        Self {
            ram: [0; 0x1000],
            bank: None,
        }
    }

    /// Unpack color from pen value (MAME prot_pvc.cpp lines 32-45).
    fn unpack_color(&mut self) {
        let pen = self.ram[0xFF0];

        let b = ((pen & 0x000F) << 1) | ((pen & 0x1000) >> 12);
        let g = ((pen & 0x00F0) >> 3) | ((pen & 0x2000) >> 13);
        let r = ((pen & 0x0F00) >> 7) | ((pen & 0x4000) >> 14);
        let s = (pen & 0x8000) >> 15;

        self.ram[0xFF1] = (g << 8) | b;
        self.ram[0xFF2] = (s << 8) | r;
    }

    /// Pack color from RGB components (MAME prot_pvc.cpp lines 49-60).
    fn pack_color(&mut self) {
        let gb = self.ram[0xFF4];
        let sr = self.ram[0xFF5];

        // Truncate to u16 like C++ does
        let result: u16 = ((gb & 0x001E) >> 1)
            | ((gb & 0x1E00) >> 5)
            | ((sr & 0x001E) << 7)
            | ((gb & 0x0001) << 12)
            | ((gb & 0x0100) << 5)
            | ((sr & 0x0001) << 14)
            | ((sr & 0x0100) << 7);

        self.ram[0xFF6] = result;
    }

    /// Get bank base address (MAME prot_pvc.cpp lines 72-80).
    fn get_bank_base(&mut self) -> u32 {
        let bankaddress: u32 = ((self.ram[0xFF8] >> 8) as u32) | ((self.ram[0xFF9] as u32) << 8);

        // Clear bits and set marker (MAME lines 74-75)
        self.ram[0xFF8] = (self.ram[0xFF8] & 0xFE00) | 0x00A0;
        self.ram[0xFF9] &= 0x7FFF;

        bankaddress + 0x100000
    }
}

impl CartPort for Pvc {
    fn read(&mut self, a: u32, _p: &[u8], _wram: &[u8]) -> Option<u16> {
        // PVC protection RAM at $2FE000-$2FFFFF (MAME neogeo.cpp lines 1231-1243)
        if (0x2FE000..=0x2FFFFF).contains(&a) {
            let offset = ((a - 0x2FE000) >> 1) as usize;
            Some(self.ram[offset])
        } else {
            None
        }
    }

    fn write(&mut self, a: u32, d: u16, upper: bool, lower: bool, _p: &[u8], _wram: &[u8]) -> bool {
        // PVC protection RAM at $2FE000-$2FFFFF (MAME neogeo.cpp lines 1480-1486)
        if (0x2FE000..=0x2FFFFF).contains(&a) {
            let offset = ((a - 0x2FE000) >> 1) as usize;

            // COMBINE_DATA: handle byte writes to 16-bit memory
            self.ram[offset] = combine_data(self.ram[offset], d, upper, lower);

            // Trigger color unpack on write to offset 0xFF0 (MAME prot_pvc.cpp line 85)
            if offset == 0xFF0 {
                self.unpack_color();
            } else if (0xFF4..=0xFF5).contains(&offset) {
                // Trigger color pack on writes to offsets 0xFF4-0xFF5 (MAME lines 86-87)
                self.pack_color();
            }

            // Bank switch on writes to offsets > 0xFF8 (MAME neogeo.cpp line 1483)
            if offset > 0xFF8 {
                self.bank = Some(self.get_bank_base());
            }

            true
        } else {
            false
        }
    }

    fn take_bank(&mut self) -> Option<u32> {
        self.bank.take()
    }

    fn reset(&mut self) {
        self.ram = [0; 0x1000];
        self.bank = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_protection_ram() {
        let mut prot = Pvc::new();
        // Write a value, then read it back
        prot.write(0x2FE000, 0xABCD, true, true, &[], &[]);
        assert_eq!(prot.read(0x2FE000, &[], &[]), Some(0xABCD));
    }

    #[test]
    fn read_outside_range_returns_none() {
        let mut prot = Pvc::new();
        assert_eq!(prot.read(0x2FD000, &[], &[]), None);
        assert_eq!(prot.read(0x300000, &[], &[]), None);
    }

    #[test]
    fn write_outside_range_not_consumed() {
        let mut prot = Pvc::new();
        assert!(!prot.write(0x2FD000, 0x1234, true, true, &[], &[]));
    }

    #[test]
    fn unpack_color_7fff() {
        // Test from task spec: write(0x2FFFE0, 0x7FFF) at offset 0xFF0
        let mut prot = Pvc::new();
        prot.write(0x2FFFE0, 0x7FFF, true, true, &[], &[]);

        // Expected values from task spec:
        // b = ((0xF)<<1)|((0x1000)>>12) = 0x1E|1 = 0x1F
        // g = ((0xF0)>>3)|((0x2000)>>13) = 0x1E|1 = 0x1F
        // r = ((0xF00)>>7)|((0x4000)>>14) = 0x1E|1 = 0x1F
        // s = (0x8000)>>15 = 0
        // ram[0xFF1] = (g<<8)|b = 0x1F1F
        // ram[0xFF2] = (s<<8)|r = 0x001F
        assert_eq!(prot.ram[0xFF1], 0x1F1F);
        assert_eq!(prot.ram[0xFF2], 0x001F);
    }

    #[test]
    fn pack_color() {
        // Test from task spec: ram[0xFF4]=0x1F1F, ram[0xFF5]=0x001F
        let mut prot = Pvc::new();
        prot.ram[0xFF4] = 0x1F1F;
        prot.ram[0xFF5] = 0x001F;

        // Trigger pack by writing to offset 0xFF4
        prot.write(0x2FFFE8, 0x1F1F, true, true, &[], &[]);

        // Calculate expected value manually:
        // gb=0x1F1F, sr=0x001F
        // ((gb&0x001E)>>1) = (0x1E>>1) = 0x0F
        // ((gb&0x1E00)>>5) = (0x1E00>>5) = 0x78
        // ((sr&0x001E)<<7) = (0x1E<<7) = 0xF00
        // ((gb&0x0001)<<12) = (0x1<<12) = 0x1000
        // ((gb&0x0100)<<5) = (0x100<<5) = 0x2000
        // ((sr&0x0001)<<14) = (0x1<<14) = 0x4000
        // ((sr&0x0100)<<7) = (0x0<<7) = 0x0
        // Total: 0x0F|0x78|0xF00|0x1000|0x2000|0x4000|0x0 = 0x7FFF
        assert_eq!(prot.ram[0xFF6], 0x7FFF);
    }

    #[test]
    fn bank_switch() {
        // Test from task spec: ram[0xFF8]=0x0200, then write to trigger bank switch
        let mut prot = Pvc::new();

        // Write high byte of bank address (offset 0xFF8)
        prot.write(0x2FFFF0, 0x0200, true, true, &[], &[]);
        assert_eq!(prot.ram[0xFF8], 0x0200);

        // Write low byte of bank address (offset 0xFF9) - this triggers bank switch
        prot.write(0x2FFFF2, 0x0001, true, true, &[], &[]);
        assert_eq!(prot.ram[0xFF9], 1);

        // Expected: bankaddress = (0x0200>>8)|(1<<8) = 0x02|0x100 = 0x102
        // After switch: ram[0xFF8] = (0x0200&0xFE00)|0xA0 = 0x02A0
        //               ram[0xFF9] = 1 & 0x7FFF = 1
        assert_eq!(prot.ram[0xFF8], 0x02A0);
        assert_eq!(prot.ram[0xFF9], 1);

        // take_bank should return bankaddress + 0x100000 = 0x102 + 0x100000 = 0x100102
        assert_eq!(prot.take_bank(), Some(0x100102));
    }

    #[test]
    fn byte_write_combining() {
        // Test COMBINE_DATA behavior with byte writes
        let mut prot = Pvc::new();

        // Write upper byte only (value in bits 8-15)
        prot.write(0x2FE000, 0xAB00, true, false, &[], &[]);
        assert_eq!(prot.ram[0], 0xAB00);

        // Write lower byte only (value in bits 0-7) - should combine with existing upper byte
        prot.write(0x2FE000, 0x00CD, false, true, &[], &[]);
        assert_eq!(prot.ram[0], 0xABCD);
    }

    #[test]
    fn reset_clears_state() {
        let mut prot = Pvc::new();
        prot.write(0x2FE000, 0x1234, true, true, &[], &[]);
        assert_eq!(prot.read(0x2FE000, &[], &[]), Some(0x1234));

        prot.reset();
        assert_eq!(prot.read(0x2FE000, &[], &[]), Some(0));
    }
}

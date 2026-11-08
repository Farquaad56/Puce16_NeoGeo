//! Memory card slot $800000-$BFFFFF (stub: card absent).
//! Real implementation is task T902. Reads return 0xFFFF, writes ignored.

use crate::device::Device;

pub struct MemCardStub;

impl Device for MemCardStub {
    fn read(&mut self, _addr: u32) -> u16 {
        // Card absent: open bus reads as all ones
        0xFFFF
    }

    fn write(&mut self, _addr: u32, _data: u16, _upper: bool, _lower: bool) {
        // Writes to absent card are ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reads_ffff() {
        let mut card = MemCardStub;
        assert_eq!(card.read(0x800000), 0xFFFF);
        assert_eq!(card.read(0xA00000), 0xFFFF);
    }

    #[test]
    fn stub_writes_ignored() {
        let mut card = MemCardStub;
        card.write(0x800000, 0x1234, true, true);
        // No observable effect - still reads as FFFF
        assert_eq!(card.read(0x800000), 0xFFFF);
    }
}

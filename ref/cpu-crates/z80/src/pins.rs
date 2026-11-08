//! Z80 pin state representation (64-bit, positive logic).
//!
//! All signals are represented as bits set to 1 = active, even for pins that
//! are physically active-low on the real Z80. This is a logical abstraction
//! used by both the CPU core (`tick`) and host emulators.

/// Pin state: 64-bit value encoding all Z80 bus signals in positive logic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pins(pub u64);

impl Pins {
    /// Address bus mask (bits 0..15).
    pub const A_MASK: u64 = 0xFFFF;
    /// Data bus shift amount.
    pub const D_SHIFT: u32 = 16;
    /// Data bus mask (bits 16..23).
    pub const D_MASK: u64 = 0xFF << 16;

    /// M1 signal mask (bit 24) - first cycle of instruction fetch.
    pub const M1: u64 = 1 << 24;
    /// Memory request mask (bit 25).
    pub const MREQ: u64 = 1 << 25;
    /// I/O request mask (bit 26).
    pub const IORQ: u64 = 1 << 26;
    /// Read signal mask (bit 27).
    pub const RD: u64 = 1 << 27;
    /// Write signal mask (bit 28).
    pub const WR: u64 = 1 << 28;

    /// HALT signal mask (bit 29).
    pub const HALT: u64 = 1 << 29;
    /// WAIT input mask (bit 30) - host requests wait state.
    pub const WAIT: u64 = 1 << 30;
    /// INT input mask (bit 31).
    pub const INT: u64 = 1 << 31;
    /// NMI input mask (bit 32).
    pub const NMI: u64 = 1 << 32;

    /// Refresh output mask (bit 33).
    pub const RFSH: u64 = 1 << 33;
    /// Bus request input mask (bit 34).
    pub const BUSRQ: u64 = 1 << 34;
    /// Bus acknowledge output mask (bit 35).
    pub const BUSAK: u64 = 1 << 35;
    /// Reset input mask (bit 36).
    pub const RESET: u64 = 1 << 36;

    /// Read the 16-bit address bus value.
    #[inline]
    pub const fn addr(self) -> u16 {
        (self.0 & Self::A_MASK) as u16
    }

    /// Read the 8-bit data bus value.
    #[inline]
    pub const fn data(self) -> u8 {
        ((self.0 & Self::D_MASK) >> Self::D_SHIFT) as u8
    }

    /// Set the address bus to `a`, preserving all other signals.
    #[inline]
    pub const fn set_addr(self, a: u16) -> Pins {
        let cleared = self.0 & !Self::A_MASK;
        Pins(cleared | (a as u64))
    }

    /// Set the data bus to `d`, preserving all other signals.
    #[inline]
    pub const fn set_data(self, d: u8) -> Pins {
        let cleared = self.0 & !Self::D_MASK;
        Pins(cleared | ((d as u64) << Self::D_SHIFT))
    }

    /// Check if all bits in `mask` are set (active).
    #[inline]
    pub const fn has(self, mask: u64) -> bool {
        (self.0 & mask) == mask
    }

    /// Set the bits specified by `mask`.
    #[inline]
    pub const fn with(self, mask: u64) -> Pins {
        Pins(self.0 | mask)
    }

    /// Clear the bits specified by `mask`.
    #[inline]
    pub const fn without(self, mask: u64) -> Pins {
        Pins(self.0 & !mask)
    }

    /// Check if this is a memory read cycle (MREQ and RD active).
    #[inline]
    pub const fn is_mem_read(self) -> bool {
        self.has(Self::MREQ | Self::RD)
    }

    /// Check if this is a memory write cycle (MREQ and WR active).
    #[inline]
    pub const fn is_mem_write(self) -> bool {
        self.has(Self::MREQ | Self::WR)
    }

    /// Check if this is an I/O read cycle (IORQ and RD active).
    #[inline]
    pub const fn is_io_read(self) -> bool {
        self.has(Self::IORQ | Self::RD)
    }

    /// Check if this is an I/O write cycle (IORQ and WR active).
    #[inline]
    pub const fn is_io_write(self) -> bool {
        self.has(Self::IORQ | Self::WR)
    }

    /// Check if this is an instruction fetch cycle (M1, MREQ, RD all active).
    #[inline]
    pub const fn is_fetch(self) -> bool {
        self.has(Self::M1 | Self::MREQ | Self::RD)
    }

    /// Check if this is an interrupt acknowledge cycle (M1 and IORQ active).
    #[inline]
    pub const fn is_int_ack(self) -> bool {
        self.has(Self::M1 | Self::IORQ)
    }
}

impl core::fmt::LowerHex for Pins {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e01_05_addr_data_roundtrip() {
        let base = Pins(0).with(Pins::MREQ);
        for addr in [0x0000u16, 0xFFFF, 0x1234] {
            let p = base.set_addr(addr);
            assert_eq!(p.addr(), addr);
            assert!(p.has(Pins::MREQ));
        }
        for data in [0x00u8, 0xFF, 0xA5] {
            let p = base.set_data(data);
            assert_eq!(p.data(), data);
            assert!(p.has(Pins::MREQ));
        }
    }

    #[test]
    fn e01_05_masks_distinct() {
        let masks = [
            Pins::A_MASK,
            Pins::D_MASK,
            Pins::M1,
            Pins::MREQ,
            Pins::IORQ,
            Pins::RD,
            Pins::WR,
            Pins::HALT,
            Pins::WAIT,
            Pins::INT,
            Pins::NMI,
            Pins::RFSH,
            Pins::BUSRQ,
            Pins::BUSAK,
            Pins::RESET,
        ];
        for i in 0..masks.len() {
            for j in (i + 1)..masks.len() {
                assert_eq!(
                    masks[i] & masks[j],
                    0,
                    "overlapping masks at {} and {}",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn e01_05_fetch_predicate() {
        let fetch = Pins(0).with(Pins::M1 | Pins::MREQ | Pins::RD);
        assert!(fetch.is_fetch());
        let mem_read = Pins(0).with(Pins::MREQ | Pins::RD);
        assert!(!mem_read.is_fetch());
        assert!(mem_read.is_mem_read());
    }

    #[test]
    fn e01_05_int_ack_predicate() {
        let int_ack = Pins(0).with(Pins::M1 | Pins::IORQ);
        assert!(int_ack.is_int_ack());
        let io_read = Pins(0).with(Pins::IORQ | Pins::RD);
        assert!(io_read.is_io_read());
    }

    #[test]
    fn e01_05_with_without() {
        let original = Pins(0x123456789ABCDEF0);
        let modified = original.with(Pins::MREQ | Pins::RD).without(Pins::WR);
        assert!(modified.has(Pins::MREQ));
        assert!(modified.has(Pins::RD));
        assert!(!modified.has(Pins::WR));
    }

    #[test]
    fn e01_05_hex_fmt() {
        let p = Pins(0x1F);
        assert_eq!(std::format!("{:x}", p), "0x1f");
    }
}

//! Pins of the 68000, packed in a `u64` (positive logic, see spec 11).
//!
//! Bit layout:
//!
//! | Bits | Name | Direction |
//! |---|---|---|
//! | 0..23 | `ADDR` (bit 0 always 0) | out |
//! | 24..39 | `DATA` | in/out |
//! | 40 | `AS` | out |
//! | 41 | `UDS` | out |
//! | 42 | `LDS` | out |
//! | 43 | `RW` (1 = read, 0 = write) | out |
//! | 44 | `DTACK` | in |
//! | 45..47 | `FC2..FC0` | out |
//!
//! | 48..50 | `IPL0..IPL2` (interrupt level, 0 = none, 7 = NMI) | in |
//! | 51 | `VPA` | in |
//! | 52 | `RESET` | in |
//! | 53 | `HALT` | in/out |
//!
//! Bits 54..63 are reserved: do not reuse.

/// Mask of the 24-bit address field (bits 0..23).
pub const ADDR_MASK: u64 = 0x00FF_FFFF;
/// Mask applied to a byte address: 24 bits, bit 0 forced to 0.
pub const ADDR_EVEN_MASK: u32 = 0x00FF_FFFE;
/// Bit position of the data bus (bits 24..39).
pub const DATA_SHIFT: u32 = 24;
/// Mask of the data bus once shifted down.
pub const DATA_MASK: u64 = 0xFFFF;
/// Address Strobe (bit 40).
pub const AS: u64 = 1 << 40;
/// Upper Data Strobe (bit 41).
pub const UDS: u64 = 1 << 41;
/// Lower Data Strobe (bit 42).
pub const LDS: u64 = 1 << 42;
/// Read/Write: 1 = read, 0 = write (bit 43).
pub const RW: u64 = 1 << 43;
/// Data Transfer Acknowledge (bit 44).
pub const DTACK: u64 = 1 << 44;
/// Bit position of the function code (bits 45..47).
pub const FC_SHIFT: u32 = 45;
/// Mask of the function code once shifted down.
pub const FC_MASK: u64 = 0b111;

/// Bit position of the interrupt priority level (bits 48..50).
pub const IPL_SHIFT: u32 = 48;
/// Valid Peripheral Address (bit 51): autovector request during an interrupt acknowledge.
pub const VPA: u64 = 1 << 51;
/// RESET (bit 52).
pub const RESET: u64 = 1 << 52;
/// HALT (bit 53).
pub const HALT: u64 = 1 << 53;

/// State of the CPU pins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pins(pub u64);

impl Pins {
    #[inline]
    fn flag(self, bit: u64) -> bool {
        self.0 & bit != 0
    }

    #[inline]
    fn with_flag(self, bit: u64, on: bool) -> Self {
        if on {
            Pins(self.0 | bit)
        } else {
            Pins(self.0 & !bit)
        }
    }

    /// 24-bit byte address (bit 0 is 0).
    #[inline]
    pub fn addr(self) -> u32 {
        (self.0 & ADDR_MASK) as u32
    }

    /// Sets the address, masked to 24 bits with bit 0 forced to 0.
    #[inline]
    pub fn with_addr(self, addr: u32) -> Self {
        Pins((self.0 & !ADDR_MASK) | u64::from(addr & ADDR_EVEN_MASK))
    }

    /// In-place version of [`Pins::with_addr`].
    #[inline]
    pub fn set_addr(&mut self, addr: u32) {
        *self = self.with_addr(addr);
    }

    /// 16-bit data bus.
    #[inline]
    pub fn data(self) -> u16 {
        ((self.0 >> DATA_SHIFT) & DATA_MASK) as u16
    }

    /// Sets the data bus.
    #[inline]
    pub fn with_data(self, data: u16) -> Self {
        Pins((self.0 & !(DATA_MASK << DATA_SHIFT)) | (u64::from(data) << DATA_SHIFT))
    }

    /// In-place version of [`Pins::with_data`].
    #[inline]
    pub fn set_data(&mut self, data: u16) {
        *self = self.with_data(data);
    }

    /// Address Strobe.
    #[inline]
    pub fn as_(self) -> bool {
        self.flag(AS)
    }

    /// Sets Address Strobe.
    #[inline]
    pub fn with_as(self, on: bool) -> Self {
        self.with_flag(AS, on)
    }

    /// Upper Data Strobe.
    #[inline]
    pub fn uds(self) -> bool {
        self.flag(UDS)
    }

    /// Sets Upper Data Strobe.
    #[inline]
    pub fn with_uds(self, on: bool) -> Self {
        self.with_flag(UDS, on)
    }

    /// Lower Data Strobe.
    #[inline]
    pub fn lds(self) -> bool {
        self.flag(LDS)
    }

    /// Sets Lower Data Strobe.
    #[inline]
    pub fn with_lds(self, on: bool) -> Self {
        self.with_flag(LDS, on)
    }

    /// True for a read cycle (RW = 1).
    #[inline]
    pub fn rw_read(self) -> bool {
        self.flag(RW)
    }

    /// Sets RW (true = read).
    #[inline]
    pub fn with_rw_read(self, read: bool) -> Self {
        self.with_flag(RW, read)
    }

    /// Data Transfer Acknowledge.
    #[inline]
    pub fn dtack(self) -> bool {
        self.flag(DTACK)
    }

    /// Sets DTACK.
    #[inline]
    pub fn with_dtack(self, on: bool) -> Self {
        self.with_flag(DTACK, on)
    }

    /// Function code FC2..FC0 (0..=7).
    #[inline]
    pub fn fc(self) -> u8 {
        ((self.0 >> FC_SHIFT) & FC_MASK) as u8
    }

    /// Sets the function code (only the low 3 bits are kept).
    #[inline]
    pub fn with_fc(self, fc: u8) -> Self {
        Pins((self.0 & !(FC_MASK << FC_SHIFT)) | ((u64::from(fc) & FC_MASK) << FC_SHIFT))
    }

    /// In-place version of [`Pins::with_fc`].
    #[inline]
    pub fn set_fc(&mut self, fc: u8) {
        *self = self.with_fc(fc);
    }

    /// Interrupt priority level on IPL0-2 (0..=7).
    #[inline]
    pub fn ipl(self) -> u8 {
        ((self.0 >> IPL_SHIFT) & 7) as u8
    }

    /// Sets the interrupt priority level (only the low 3 bits are kept).
    #[inline]
    pub fn with_ipl(self, level: u8) -> Self {
        Pins((self.0 & !(7 << IPL_SHIFT)) | ((u64::from(level) & 7) << IPL_SHIFT))
    }

    /// VPA input.
    #[inline]
    pub fn vpa(self) -> bool {
        self.flag(VPA)
    }

    /// Sets VPA.
    #[inline]
    pub fn with_vpa(self, on: bool) -> Self {
        self.with_flag(VPA, on)
    }

    /// RESET input.
    #[inline]
    pub fn reset(self) -> bool {
        self.flag(RESET)
    }

    /// Sets RESET.
    #[inline]
    pub fn with_reset(self, on: bool) -> Self {
        self.with_flag(RESET, on)
    }

    /// HALT line.
    #[inline]
    pub fn halt(self) -> bool {
        self.flag(HALT)
    }

    /// Sets HALT.
    #[inline]
    pub fn with_halt(self, on: bool) -> Self {
        self.with_flag(HALT, on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e02_01_layout() {
        assert_eq!(Pins(0).with_as(true).0, 1 << 40);
        assert_eq!(Pins(0).with_rw_read(true).0, 1 << 43);
        assert_eq!(Pins(0).with_dtack(true).0, 1 << 44);
        assert_eq!(Pins(0).with_uds(true).0, 1 << 41);
        assert_eq!(Pins(0).with_lds(true).0, 1 << 42);
    }

    #[test]
    fn e02_01_addr_mask() {
        assert_eq!(Pins(0).with_addr(0x0123_4567).addr(), 0x0023_4566);
    }

    #[test]
    fn e02_01_data_roundtrip() {
        let base = Pins(0).with_as(true).with_fc(5).with_addr(0x1000);
        let p = base.with_data(0xBEEF);
        assert_eq!(p.data(), 0xBEEF);
        assert_eq!(p.with_data(0).0, base.0);
    }

    #[test]
    fn e02_01_fc_roundtrip() {
        assert_eq!(Pins(0).with_fc(6).fc(), 6);
        assert_eq!(Pins(0).with_fc(0xFF).fc(), 7);
    }

    #[test]
    fn e02_01_independent() {
        let p = Pins(0)
            .with_addr(0x00AB_CDE0)
            .with_data(0x1234)
            .with_as(true)
            .with_uds(true)
            .with_lds(false)
            .with_rw_read(true)
            .with_dtack(true)
            .with_fc(6);
        assert_eq!(p.addr(), 0x00AB_CDE0);
        assert_eq!(p.data(), 0x1234);
        assert!(p.as_() && p.uds() && !p.lds() && p.rw_read() && p.dtack());
        assert_eq!(p.fc(), 6);
        let q = p.with_as(false);
        assert_eq!(q.addr(), 0x00AB_CDE0);
        assert_eq!(q.data(), 0x1234);
        assert!(!q.as_() && q.uds() && q.rw_read() && q.dtack());
        assert_eq!(q.fc(), 6);
        // reserved bits 48..53 are never touched
        assert_eq!(p.0 >> 48, 0);
    }

    #[test]
    fn e11_05_ipl_pins() {
        let p = Pins::default()
            .with_ipl(5)
            .with_vpa(true)
            .with_reset(true)
            .with_halt(true);
        assert_eq!(
            (p.ipl(), p.vpa(), p.reset(), p.halt()),
            (5, true, true, true)
        );
        assert_eq!(p.with_ipl(2).ipl(), 2);
        assert_eq!(p.addr(), 0);
        assert_eq!(p.fc(), 0);
    }

    #[test]
    fn e02_01_default_zero() {
        assert_eq!(Pins::default().0, 0);
    }
}

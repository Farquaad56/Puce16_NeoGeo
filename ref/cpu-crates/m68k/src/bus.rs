//! Convenience bus interface for hosts that do not want to drive the pins by hand.

use crate::pins::Pins;

/// Kind of bus cycle the core is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    /// Instruction prefetch (program space, FC = 010 or 110).
    Fetch,
    /// Data read.
    Read,
    /// Data write.
    Write,
    /// Indivisible read-modify-write cycle of TAS.
    Tas,
    /// Internal cycle without bus activity.
    Internal,
}

/// Memory/peripheral side of the CPU.
///
/// `uds`/`lds` select the byte lanes: a word access sets both, a byte access at an even
/// address sets only `uds` (data on D8..D15), at an odd address only `lds` (D0..D7).
pub trait Bus {
    /// Reads a 16-bit word (`addr` is even; the strobes select the byte lanes).
    fn read(&mut self, addr: u32, fc: u8, uds: bool, lds: bool) -> u16;
    /// Writes a 16-bit word (only the lanes selected by the strobes are stored).
    fn write(&mut self, addr: u32, fc: u8, uds: bool, lds: bool, data: u16);
    /// Extra wait clocks for the cycle that starts now (default 0 = DTACK immediate).
    fn wait_clocks(&mut self, _addr: u32, _write: bool) -> u8 {
        0
    }
    /// Interrupt priority level on IPL0-2 sampled at the instruction boundary (0 = none).
    fn ipl(&mut self) -> u8 {
        0
    }
    /// Interrupt acknowledge cycle (FC = 7) of `level`: the vector number the host supplies, or
    /// `None` to assert VPA (autovector 24 + level).
    fn int_ack(&mut self, _level: u8) -> Option<u8> {
        None
    }
    /// Called once per clock with the pins the CPU drives (peripheral clocks).
    fn on_tick(&mut self, _pins: Pins) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram {
        last: Option<(u32, u16)>,
    }

    impl Bus for Ram {
        fn read(&mut self, _addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
            0x1234
        }
        fn write(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool, data: u16) {
            self.last = Some((addr, data));
        }
    }

    #[test]
    fn e02_03_defaults() {
        let mut b = Ram { last: None };
        assert_eq!(b.wait_clocks(0x1000, false), 0);
        assert_eq!(b.wait_clocks(0x1000, true), 0);
        b.on_tick(Pins::default());
        assert_eq!(b.read(0, 6, true, true), 0x1234);
        b.write(2, 5, true, true, 0xBEEF);
        assert_eq!(b.last, Some((2, 0xBEEF)));
    }

    #[test]
    fn e02_03_object_safe() {
        fn run(bus: &mut dyn Bus) -> u16 {
            bus.on_tick(Pins::default());
            bus.read(0, 6, true, true)
        }
        let mut b = Ram { last: None };
        assert_eq!(run(&mut b), 0x1234);
        assert_eq!(Cycle::Fetch, Cycle::Fetch);
        assert_ne!(Cycle::Read, Cycle::Tas);
    }
}

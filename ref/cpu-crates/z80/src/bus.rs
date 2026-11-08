//! Bus trait and SimpleRam test implementation.
//!
//! The `Bus` trait provides a convenient interface for hosts that do not need
//! to manipulate individual pins. This is the most commonly used API for
//! console and arcade emulators.

use crate::pins::Pins;

/// Types of CPU bus cycles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    /// Opcode fetch (M1 cycle).
    Fetch,
    /// Memory read.
    MemRead,
    /// Memory write.
    MemWrite,
    /// I/O port read.
    IoRead,
    /// I/O port write.
    IoWrite,
    /// INT acknowledge cycle.
    IntAck,
    /// Internal CPU cycle (no bus activity).
    Internal,
}

/// Convenience bus interface for hosts that do not manipulate pins directly.
///
/// Required methods: `read` and `write`. All other methods have sensible
/// defaults for a minimal CPU-only system.
pub trait Bus {
    /// Read a byte from memory at `addr`.
    fn read(&mut self, addr: u16) -> u8;

    /// Write `data` to memory at `addr`.
    fn write(&mut self, addr: u16, data: u8);

    /// Read a byte from I/O port `port`.
    /// Default: returns 0xFF (no devices connected).
    fn io_read(&mut self, _port: u16) -> u8 {
        0xFF
    }

    /// Write `data` to I/O port `port`.
    /// Default: no-op (no devices connected).
    fn io_write(&mut self, _port: u16, _data: u8) {}

    /// Place an interrupt vector byte on the bus during INT acknowledge.
    /// Default: returns 0xFF (IM0/IM1 behavior).
    fn int_ack(&mut self) -> u8 {
        0xFF
    }

    /// Return the number of extra wait T-states for this cycle.
    /// Used for memory contention (ZX Spectrum) and arcade WAIT signals.
    /// Default: 0 (no wait states).
    fn extra_wait(&mut self, _kind: Cycle, _addr: u16) -> u8 {
        0
    }

    /// Return true if an IRQ request is currently active.
    /// Default: false (no interrupts).
    fn irq(&mut self) -> bool {
        false
    }

    /// Return true if an NMI request is currently active.
    /// Default: false (no NMI).
    fn nmi(&mut self) -> bool {
        false
    }

    /// Return true if a BUSRQ request is currently active.
    /// Default: false (no bus request).
    fn busrq(&mut self) -> bool {
        false
    }

    /// Called at every T-state with the current pin state.
    /// Used to clock peripherals (VDP, sound, CTC) at T-state granularity.
    /// Default: no-op.
    fn on_tick(&mut self, _pins: Pins) {}
}

/// Simple 64KB RAM implementation of the Bus trait for testing.
///
/// Memory is on the stack (64KB is acceptable). For heap allocation,
/// use `SimpleRam::boxed()` (requires the `std` feature).
pub struct SimpleRam {
    /// The 64KB memory array.
    pub mem: [u8; 65536],
}

impl SimpleRam {
    /// Create a new SimpleRam with all memory zeroed.
    pub fn new() -> SimpleRam {
        SimpleRam { mem: [0; 65536] }
    }

    /// Create a heap-allocated SimpleRam (requires `std` feature).
    #[cfg(feature = "std")]
    pub fn boxed() -> std::boxed::Box<SimpleRam> {
        std::boxed::Box::new(SimpleRam::new())
    }
}

impl Default for SimpleRam {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for SimpleRam {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.mem[addr as usize] = data;
    }

    fn io_read(&mut self, _port: u16) -> u8 {
        0xFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e03_11_defaults() {
        // A minimal Bus that only defines read/write
        struct MinimalBus;
        impl Bus for MinimalBus {
            fn read(&mut self, _addr: u16) -> u8 {
                0x42
            }
            fn write(&mut self, _addr: u16, _data: u8) {}
        }

        let mut bus = MinimalBus;
        assert_eq!(bus.io_read(0), 0xFF);
        assert_eq!(bus.int_ack(), 0xFF);
        assert_eq!(bus.extra_wait(Cycle::Fetch, 0), 0);
        assert!(!bus.irq());
        assert!(!bus.nmi());
        assert!(!bus.busrq());
    }

    #[test]
    fn e03_11_ram_rw() {
        let mut ram = SimpleRam::new();
        ram.write(0x1000, 0xAB);
        assert_eq!(ram.read(0x1000), 0xAB);
        ram.write(0x2000, 0xCD);
        assert_eq!(ram.read(0x2000), 0xCD);
        assert_eq!(ram.read(0x1000), 0xAB);
    }

    #[test]
    fn e03_11_cycle_enum() {
        // Verify Cycle derives Debug and PartialEq
        let c1 = Cycle::Fetch;
        let c2 = Cycle::MemRead;
        assert_ne!(c1, c2);
        assert_eq!(std::format!("{:?}", c1), "Fetch");
        assert_eq!(std::format!("{:?}", c2), "MemRead");
    }

    #[test]
    fn e03_11_ram_zero() {
        let ram = SimpleRam::new();
        let mut all_zero = true;
        for i in 0..65536 {
            if ram.mem[i] != 0 {
                all_zero = false;
                break;
            }
        }
        assert!(all_zero);
    }
}

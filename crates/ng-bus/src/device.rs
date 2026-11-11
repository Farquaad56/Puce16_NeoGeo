//! Device trait and null device for bus testing.

/// A memory-mapped device on the 68000 bus.
pub trait Device {
    /// Read a 16-bit word at address `addr` (24-bit, even).
    fn read(&mut self, addr: u32) -> u16;
    /// Write a 16-bit word at address `addr`.
    /// `upper`/`lower` indicate which byte(s) are being written.
    fn write(&mut self, addr: u32, data: u16, upper: bool, lower: bool);
}

/// Cart port protection chip interface (e.g., CM-C, CM-F).
pub trait CartPort {
    /// Intercept a read at address `a` in the $200000-$2FFFFF window.
    /// Return Some(value) to override normal ROM access.
    fn read(&mut self, a: u32, p: &[u8], wram: &[u8]) -> Option<u16>;
    /// Intercept a write at address `a`.
    /// Return true if the write was consumed (no further action needed).
    fn write(&mut self, a: u32, d: u16, upper: bool, lower: bool, p: &[u8], wram: &[u8]) -> bool;
    /// After a consumed write, check if the port requested a bank switch.
    fn take_bank(&mut self) -> Option<u32> {
        None
    }
    /// Reset the protection chip state.
    fn reset(&mut self) {}
    /// RAM non volatile de la cartouche (V-Liner, Jockey GP), s'il y en a une.
    fn nv_ram(&self) -> Option<&[u8]> {
        None
    }
    /// Accès en écriture à la RAM non volatile (chargement d'une sauvegarde / savestate).
    fn nv_ram_mut(&mut self) -> Option<&mut [u8]> {
        None
    }
    /// Nombre d'accès traités par la puce depuis sa création (tests de validation).
    fn access_count(&self) -> u64 {
        0
    }
    /// Nombre de changements de banque demandés par la puce depuis sa création (tests de validation).
    fn bank_change_count(&self) -> u64 {
        0
    }
}

/// Null device: reads return 0xFFFF, writes are ignored.
pub struct NullDevice;

impl Device for NullDevice {
    fn read(&mut self, _addr: u32) -> u16 {
        0xFFFF
    }
    fn write(&mut self, _addr: u32, _data: u16, _upper: bool, _lower: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_device_reads_ffff() {
        let mut dev = NullDevice;
        assert_eq!(dev.read(0x300000), 0xFFFF);
    }

    #[test]
    fn null_device_writes_ignored() {
        let mut dev = NullDevice;
        dev.write(0x300000, 0x1234, true, true);
        // No observable effect
    }
}

//! Neo Geo AES (console) differences from MVS arcade board.
//!
//! Per docs/spec/05-boot-io-system.md §Reset / boot (AES):
//! - No backup RAM: $D00000-$FFFFFF => open bus
//! - Different I/O register reads at $300000, $340000, $380000, $320000
//! - Bit 15 of REG_STATUS_B ($380000) = 0 (MVS has it as 1)
//! - No watchdog on AES

use crate::device::Device;
use crate::io::Io;
use ng_cart::Model;

/// Create an Io device configured for the given model.
pub fn make_io(model: Model) -> Box<dyn Device> {
    let io = Io {
        model,
        ..Default::default()
    };
    Box::new(io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map68k::Bus68k;

    #[test]
    fn aes_status_b_bit15_zero() {
        // AES: read $380000 should have bit 15 = 0 (MVS has it as 1)
        let mut io = Io {
            model: Model::Aes,
            ..Default::default()
        };
        // All buttons released, no start/select pressed
        let val = io.read(0x380000);
        assert_eq!(val & 0x8000, 0, "AES bit 15 must be 0");
        // Expected: 0x00FF | (system bits) | 0x7000 = 0x7FFF when all released
        assert_eq!(val, 0x7FFF);
    }

    #[test]
    fn aes_status_b_start1_pressed() {
        let mut io = Io {
            model: Model::Aes,
            ..Default::default()
        };
        // Start1 pressed (bit 0 of system.bits cleared)
        io.system.bits &= !(1 << 0);
        let val = io.read(0x380000);
        // Bit 8 should be clear (start1), bit 15 still 0
        assert_eq!(val & 0x8000, 0);
        assert_eq!(val & 0x0100, 0, "Start1 bit should be clear");
        assert_eq!(val, 0x7EFF);
    }

    #[test]
    fn aes_no_backup_ram() {
        // AES: reads from $D00000 should return open_bus (no backup RAM)
        let bios = vec![0u8; 0x20000];
        let cart_p = vec![0u8; 0x100000];
        let mut bus = Bus68k::new(bios, cart_p);
        bus.model = Model::Aes;
        bus.open_bus = 0xBEEF;
        // For AES, $D00000 should return open_bus
        let val = bus.read16(0xD00000);
        assert_eq!(val, 0xBEEF, "AES: $D00000 should be open bus");
    }

    #[test]
    fn aes_sound_status_no_rtc() {
        // AES: read $320000 returns (sound_reply << 8) with no RTC bits
        let mut io = Io {
            model: Model::Aes,
            sound_reply: 0x12,
            ..Default::default()
        };
        let val = io.read(0x320000);
        // Bits 0-7 should be 0 (no RTC, no coin bits)
        assert_eq!(val & 0xFF, 0, "AES: lower byte of $320000 should be 0");
        assert_eq!((val >> 8) as u8, 0x12);
    }

    #[test]
    fn aes_p1cnt_no_dipsw() {
        // AES: read $300000 returns (pad1 << 8) | 0xFF (no DIP switches)
        let mut io = Io {
            model: Model::Aes,
            ..Default::default()
        };
        let val = io.read(0x300000);
        assert_eq!(
            val & 0xFF,
            0xFF,
            "AES: lower byte of $300000 should be 0xFF"
        );
    }

    #[test]
    fn aes_p2cnt_no_dipsw() {
        // AES: read $340000 returns (pad2 << 8) | 0xFF
        let mut io = Io {
            model: Model::Aes,
            ..Default::default()
        };
        let val = io.read(0x340000);
        assert_eq!(
            val & 0xFF,
            0xFF,
            "AES: lower byte of $340000 should be 0xFF"
        );
    }

    #[test]
    fn aes_no_watchdog_kick() {
        // AES: write to $300001 should NOT trigger watchdog kick
        let mut io = Io {
            model: Model::Aes,
            ..Default::default()
        };
        io.write(0x300001, 0x0000, false, true);
        assert!(!io.watchdog_kick, "AES: no watchdog kick on $300001 write");
    }

    #[test]
    fn mvs_watchdog_still_works() {
        // MVS: write to $300001 should still trigger watchdog kick
        let mut io = Io {
            model: Model::Mvs,
            ..Default::default()
        };
        io.write(0x300001, 0x0000, false, true);
        assert!(io.watchdog_kick, "MVS: watchdog kick should still work");
    }
}

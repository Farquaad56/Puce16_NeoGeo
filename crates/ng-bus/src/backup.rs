//! Backup RAM (MVS) $D00000-$DFFFFF, 64 KiB mirrored.
//! Writes ignored while locked (Q6 latch = 0). Persistent via save/load.

use std::fs;
use std::path::Path;

use crate::device::Device;

pub struct BackupRam {
    pub data: Box<[u8; 0x10000]>,
    pub unlocked: bool,
}

impl Default for BackupRam {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupRam {
    pub fn new() -> Self {
        let mut data = [0u8; 0x10000];
        // Initialize to 0xFF like real hardware (uninitialized SRAM)
        for b in data.iter_mut() {
            *b = 0xFF;
        }
        BackupRam {
            data: Box::new(data),
            unlocked: false,
        }
    }

    /// Load backup RAM from a raw 64 KiB file. If missing, zero-fill.
    pub fn load(&mut self, path: &Path) {
        match fs::read(path) {
            Ok(bytes) if bytes.len() == 0x10000 => {
                let src: &[u8] = &bytes;
                let dst: &mut [u8] = &mut self.data[..];
                dst.copy_from_slice(src);
            }
            _ => {
                // File missing or wrong size: zero-fill
                for b in self.data.iter_mut() {
                    *b = 0x00;
                }
            }
        }
    }

    /// Save backup RAM to a raw 64 KiB file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        fs::write(path, self.data.as_ref())
    }
}

impl Device for BackupRam {
    fn read(&mut self, addr: u32) -> u16 {
        let offset = (addr & 0xFFFF) as usize;
        let hi = self.data[offset];
        let lo = self.data[offset + 1];
        ((hi as u16) << 8) | (lo as u16)
    }

    fn write(&mut self, addr: u32, data: u16, upper: bool, lower: bool) {
        if !self.unlocked {
            return;
        }
        let offset = (addr & 0xFFFF) as usize;
        if upper {
            self.data[offset] = (data >> 8) as u8;
        }
        if lower {
            self.data[offset + 1] = data as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_write_ignored() {
        let mut ram = BackupRam::new();
        ram.unlocked = false;
        // Write should be ignored when locked
        ram.write(0xD00000, 0x1234, true, true);
        assert_eq!(ram.read(0xD00000), 0xFFFF);
    }

    #[test]
    fn unlocked_write_works() {
        let mut ram = BackupRam::new();
        ram.unlocked = true;
        ram.write(0xD00000, 0x1234, true, true);
        assert_eq!(ram.read(0xD00000), 0x1234);
    }

    #[test]
    fn mirror_addressing() {
        let mut ram = BackupRam::new();
        ram.unlocked = true;
        ram.write(0xD00000, 0x1234, true, true);
        // $D10000 should read the same data (mirrored)
        assert_eq!(ram.read(0xD10000), 0x1234);
    }

    #[test]
    fn save_load_roundtrip() {
        let mut ram = BackupRam::new();
        ram.unlocked = true;
        // Write some data
        for i in 0..10 {
            ram.write(0xD00000 + (i * 2) as u32, 0xAABB, true, true);
        }

        let path = Path::new("/tmp/backup_test.sav");
        ram.save(path).unwrap();

        // Load into a fresh instance
        let mut ram2 = BackupRam::new();
        ram2.load(path);
        for i in 0..10 {
            assert_eq!(ram2.read(0xD00000 + (i * 2) as u32), 0xAABB);
        }

        fs::remove_file(path).ok();
    }

    #[test]
    fn load_missing_file_zeros() {
        let mut ram = BackupRam::new();
        // Write some data first so it's not already zeroed
        ram.unlocked = true;
        ram.write(0xD00000, 0x1234, true, true);

        let path = Path::new("/tmp/backup_missing.sav");
        fs::remove_file(path).ok(); // Ensure it doesn't exist
        ram.load(path);
        assert_eq!(ram.read(0xD00000), 0x0000);
    }
}

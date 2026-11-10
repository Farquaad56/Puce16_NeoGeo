//! Carte mémoire $800000-$BFFFFF (T903) : 2 Kio, octets impairs uniquement (MAME `memcard_r/w`).

use std::path::Path;

use crate::device::Device;

/// Taille de la carte en octets.
pub const MEMCARD_SIZE: usize = 2048;

pub struct MemCard {
    pub data: Vec<u8>,
    /// Une carte est insérée dans le connecteur.
    pub present: bool,
    /// Protection en écriture de la carte.
    pub protected: bool,
}

impl Default for MemCard {
    /// Carte absente (contenu effacé à 0xFF).
    fn default() -> Self {
        MemCard {
            data: vec![0xFF; MEMCARD_SIZE],
            present: false,
            protected: false,
        }
    }
}

impl MemCard {
    /// Charge le contenu d'un fichier de 2048 octets et insère la carte ; renvoie faux (carte
    /// inchangée) si le fichier est absent ou de mauvaise taille.
    pub fn load(&mut self, path: &Path) -> bool {
        match std::fs::read(path) {
            Ok(d) if d.len() == MEMCARD_SIZE => {
                self.data = d;
                self.present = true;
                true
            }
            _ => false,
        }
    }

    /// Écrit le contenu dans un fichier (no-op si aucune carte).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if self.present {
            std::fs::write(path, &self.data)
        } else {
            Ok(())
        }
    }
}

impl Device for MemCard {
    fn read(&mut self, addr: u32) -> u16 {
        if self.present {
            0xFF00 | u16::from(self.data[((addr >> 1) & 0x7FF) as usize])
        } else {
            0xFFFF
        }
    }

    fn write(&mut self, addr: u32, data: u16, _upper: bool, lower: bool) {
        if self.present && !self.protected && lower {
            self.data[((addr >> 1) & 0x7FF) as usize] = data as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> MemCard {
        MemCard {
            present: true,
            ..MemCard::default()
        }
    }

    #[test]
    fn absent_card_reads_ffff_and_ignores_writes() {
        let mut c = MemCard::default();
        assert_eq!(c.read(0x800000), 0xFFFF);
        c.write(0x800006, 0x00A5, false, true);
        assert_eq!(c.data[3], 0xFF);
    }

    #[test]
    fn present_card_reads_and_writes_odd_bytes() {
        let mut c = card();
        c.data[3] = 0x5A;
        assert_eq!(c.read(0x800006), 0xFF5A);
        c.write(0x800006, 0x00A5, false, true);
        assert_eq!(c.data[3], 0xA5);
        // l'octet haut seul n'écrit rien
        c.write(0x800006, 0x7700, true, false);
        assert_eq!(c.data[3], 0xA5);
    }

    #[test]
    fn protected_card_rejects_writes() {
        let mut c = card();
        c.protected = true;
        c.write(0x800000, 0x0012, false, true);
        assert_eq!(c.data[0], 0xFF);
    }

    #[test]
    fn address_mirrors_every_4_kib() {
        let mut c = card();
        c.data[0x7FF] = 0x11;
        assert_eq!(c.read(0x800FFE), 0xFF11);
        assert_eq!(c.read(0xBFFFFE), 0xFF11);
    }

    #[test]
    fn file_roundtrip() {
        let path = std::env::temp_dir().join("ng_memcard_t903.memcard");
        let mut c = card();
        c.data[10] = 0x42;
        c.save(&path).expect("écriture");
        let mut d = MemCard::default();
        assert!(d.load(&path));
        assert!(d.present);
        assert_eq!(d.data[10], 0x42);
        let _ = std::fs::remove_file(&path);
        let mut e = MemCard::default();
        assert!(!e.load(&path));
        assert!(!e.present);
    }
}

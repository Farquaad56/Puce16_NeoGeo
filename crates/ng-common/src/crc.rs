//! CRC-32/ISO-HDLC (polynôme réfléchi 0xEDB88320).
//!
//! Implémentation sans dépendance externe, table de lookup calculée au premier appel.

use std::sync::OnceLock;

/// Calcule le CRC-32 d'un tampon d'octets.
pub fn crc32_bytes(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF_u32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ table()[index];
    }
    crc ^ 0xFFFFFFFF
}

/// Table de lookup CRC-32 (256 entrées), calculée une seule fois.
fn table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let mut crc = i as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
            *entry = crc;
        }
        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_standard_vector() {
        // CRC-32/ISO-HDLC standard test vector
        assert_eq!(crc32_bytes(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32_bytes(&[]), 0x00000000);
    }

    #[test]
    fn crc32_zeros_286720() {
        // CRC of 286720 zero bytes (empty framebuffer serialized)
        let zeros = vec![0u8; 286720];
        assert_eq!(crc32_bytes(&zeros), 0xE7C953CF);
    }
}

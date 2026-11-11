//! Super Bubble Pop : patch de la ROM P à l'installation (MAME `neogeo_sbp_cart_device::patch`).
//!
//! Convention : `P` est stockée en mots 16 bits big-endian (adresse 68000 = indice d'octet), donc
//! le mot MAME `rom[x/2] = v` devient `p[x] = v >> 8 ; p[x+1] = v & 0xFF`.
//! Attention : `rom[0x3ff2d/2]` se calcule en entier (0x3FF2D / 2 = 0x1FF96) => adresse 0x3FF2C.

fn put_word(p: &mut [u8], addr: usize, v: u16) {
    if addr + 1 < p.len() {
        p[addr] = (v >> 8) as u8;
        p[addr + 1] = v as u8;
    }
}

pub fn sbp_patch(p: &mut [u8]) {
    put_word(p, 0x2A6F8, 0x4E71);
    put_word(p, 0x2A6FA, 0x4E71);
    put_word(p, 0x2A6FC, 0x4E71);
    put_word(p, 0x3FF2C, 0x7001);
}

#[cfg(test)]
mod sbp_patch_tests {
    use super::*;

    #[test]
    fn patch_on_zero_buffer() {
        let mut p = vec![0u8; 0x100000];
        sbp_patch(&mut p);
        assert_eq!(&p[0x2A6F8..0x2A6FE], &[0x4E, 0x71, 0x4E, 0x71, 0x4E, 0x71]);
        assert_eq!(&p[0x3FF2C..0x3FF2E], &[0x70, 0x01]);
        let nz = p.iter().filter(|&&b| b != 0).count();
        assert_eq!(nz, 6 + 2);
    }

    #[test]
    fn patch_short_buffer_does_not_panic() {
        let mut p = vec![0u8; 0x10];
        sbp_patch(&mut p);
        assert!(p.iter().all(|&b| b == 0));
    }
}

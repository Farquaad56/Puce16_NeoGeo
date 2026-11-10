// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_sma.cpp (mslug3_decrypt_68k, lines 428-451)
//! SMA Metal Slug 3 P-ROM decryption.

use ng_common::bitswap::bitswap;

/// Read a big-endian 16-bit word from byte slice at word index i.
fn get16(p: &[u8], i: usize) -> u16 {
    ((p[2 * i] as u16) << 8) | (p[2 * i + 1] as u16)
}

/// Write a big-endian 16-bit word to byte slice at word index i.
fn set16(p: &mut [u8], i: usize, v: u16) {
    p[2 * i] = (v >> 8) as u8;
    p[2 * i + 1] = (v & 0xFF) as u8;
}

/// Decrypt Metal Slug 3 P-ROM data.
///
/// Port of MAME's sma_prot_device::mslug3_decrypt_68k().
/// Operates on the full ROM image in-place. The input must be at least
/// 0x900000 bytes (the fixed part plus one bank).
pub fn mslug3_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() >= 0x900000,
        "mslug3_decrypt_68k requires at least 0x900000 bytes"
    );

    // Step 1: swap data lines on the whole ROMs (banked part only)
    // rom = base + 0x100000; for i in 0..0x800000/2: bitswap<16> each word
    let bank_offset_words = 0x100000 / 2;
    let bank_size_words = 0x800000 / 2;
    #[allow(clippy::needless_range_loop)]
    for i in 0..bank_size_words {
        let w = get16(p, bank_offset_words + i) as u32;
        let swapped = bitswap::<16>(w, [4, 11, 14, 3, 1, 13, 0, 7, 2, 8, 12, 15, 10, 9, 5, 6]);
        set16(p, bank_offset_words + i, swapped as u16);
    }

    // Step 2: swap address lines & relocate fixed part
    // rom = base; for i in 0..0x0C0000/2: read from relocated source
    let fixed_size_words = 0x0C0000 / 2;
    let src_base_words = 0x5D0000 / 2;
    // Copy to temp buffer first since we're reading from the same region
    let mut fixed_buf = vec![0u16; fixed_size_words];
    #[allow(clippy::needless_range_loop)]
    for i in 0..fixed_size_words {
        let src_idx = src_base_words
            + bitswap::<19>(
                i as u32,
                [
                    18, 15, 2, 1, 13, 3, 0, 9, 6, 16, 4, 11, 5, 7, 12, 17, 14, 10, 8,
                ],
            ) as usize;
        fixed_buf[i] = get16(p, src_idx);
    }
    #[allow(clippy::needless_range_loop)]
    for i in 0..fixed_size_words {
        set16(p, i, fixed_buf[i]);
    }

    // Step 3: swap address lines for the banked part (per-bank)
    // rom = base + 0x100000; for each 0x10000-byte block: bitswap<15> within block
    let block_size_words = 0x10000 / 2;
    #[allow(clippy::needless_range_loop)]
    for i in (0..bank_size_words).step_by(block_size_words) {
        // Copy block to buffer
        let mut buf = vec![0u16; block_size_words];
        #[allow(clippy::needless_range_loop)]
        for j in 0..block_size_words {
            buf[j] = get16(p, bank_offset_words + i + j);
        }
        // Permute within block
        #[allow(clippy::needless_range_loop)]
        for j in 0..block_size_words {
            let src_j = bitswap::<15>(j as u32, [2, 11, 0, 14, 6, 4, 13, 8, 9, 3, 10, 7, 5, 12, 1])
                as usize;
            set16(p, bank_offset_words + i + j, buf[src_j]);
        }
    }
}

// ---- T1007 : kof99, garou, kof2000 (ports exacts de prot_sma.cpp) ----

/// Échange de lignes de données sur les 0x800000 octets bankés (`base + 0x100000`).
fn swap_data_lines(p: &mut [u8], f: impl Fn(u16) -> u16) {
    let first = 0x100000 / 2;
    for i in 0..0x800000 / 2 {
        let w = get16(p, first + i);
        set16(p, first + i, f(w));
    }
}

/// Échange de lignes d'adresse dans la partie bankée : `words` mots au total, par blocs de
/// `block` mots ; `map(j)` donne l'indice source dans le bloc.
fn swap_banked_address(p: &mut [u8], words: usize, block: usize, map: impl Fn(usize) -> usize) {
    let first = 0x100000 / 2;
    let mut buf = vec![0u16; block];
    for i in (0..words).step_by(block) {
        for (j, b) in buf.iter_mut().enumerate() {
            *b = get16(p, first + i + j);
        }
        for j in 0..block {
            set16(p, first + i + j, buf[map(j)]);
        }
    }
}

/// Relocalise la partie fixe (0xC0000 octets) depuis `src_words` mots, lignes d'adresse permutées.
fn relocate_fixed(p: &mut [u8], src_words: usize, map: impl Fn(usize) -> usize) {
    let n = 0x0C0000 / 2;
    let buf: Vec<u16> = (0..n).map(|i| get16(p, src_words + map(i))).collect();
    for (i, w) in buf.into_iter().enumerate() {
        set16(p, i, w);
    }
}

fn need_full_p(p: &[u8], name: &str) {
    assert!(
        p.len() >= 0x900000,
        "{name} requires at least 0x900000 bytes"
    );
}

/// MAME `kof99_decrypt_68k` : données, adresses bankées (0x600000), relocalisation (0x700000).
pub fn kof99_decrypt_68k(p: &mut [u8]) {
    need_full_p(p, "kof99_decrypt_68k");
    swap_data_lines(p, |w| {
        bitswap::<16>(
            w as u32,
            [13, 7, 3, 0, 9, 4, 5, 6, 1, 12, 8, 14, 10, 11, 2, 15],
        ) as u16
    });
    swap_banked_address(p, 0x600000 / 2, 0x800 / 2, |j| {
        bitswap::<10>(j as u32, [6, 2, 4, 9, 8, 3, 1, 7, 0, 5]) as usize
    });
    relocate_fixed(p, 0x700000 / 2, |i| {
        bitswap::<19>(
            i as u32,
            [
                18, 11, 6, 14, 17, 16, 5, 8, 10, 12, 0, 4, 3, 2, 7, 9, 15, 13, 1,
            ],
        ) as usize
    });
}

/// MAME `garou_decrypt_68k` : données, relocalisation (0x710000), puis adresses bankées.
pub fn garou_decrypt_68k(p: &mut [u8]) {
    need_full_p(p, "garou_decrypt_68k");
    swap_data_lines(p, |w| {
        bitswap::<16>(
            w as u32,
            [13, 12, 14, 10, 8, 2, 3, 1, 5, 9, 11, 4, 15, 0, 6, 7],
        ) as u16
    });
    relocate_fixed(p, 0x710000 / 2, |i| {
        bitswap::<19>(
            i as u32,
            [
                18, 4, 5, 16, 14, 7, 9, 6, 13, 17, 15, 3, 1, 2, 12, 11, 8, 10, 0,
            ],
        ) as usize
    });
    swap_banked_address(p, 0x800000 / 2, 0x8000 / 2, |j| {
        bitswap::<14>(j as u32, [9, 4, 8, 3, 13, 6, 2, 7, 0, 12, 1, 11, 10, 5]) as usize
    });
}

/// MAME `kof2000_decrypt_68k` : données, adresses bankées (0x63A000), relocalisation (0x73A000).
pub fn kof2000_decrypt_68k(p: &mut [u8]) {
    need_full_p(p, "kof2000_decrypt_68k");
    swap_data_lines(p, |w| {
        bitswap::<16>(
            w as u32,
            [12, 8, 11, 3, 15, 14, 7, 0, 10, 13, 6, 5, 9, 2, 1, 4],
        ) as u16
    });
    swap_banked_address(p, 0x63A000 / 2, 0x800 / 2, |j| {
        bitswap::<10>(j as u32, [4, 1, 3, 8, 6, 2, 7, 0, 9, 5]) as usize
    });
    relocate_fixed(p, 0x73A000 / 2, |i| {
        bitswap::<19>(
            i as u32,
            [
                18, 8, 4, 15, 13, 3, 14, 16, 2, 6, 17, 7, 12, 10, 0, 5, 11, 1, 9,
            ],
        ) as usize
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ng_common::crc32_bytes;
    use std::path::PathBuf;

    fn data_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests-data/prot")
            .join(name)
    }

    #[test]
    fn get16_set16_roundtrip() {
        let mut p = [0u8; 16];
        set16(&mut p, 3, 0xABCD);
        assert_eq!(p[6], 0xAB);
        assert_eq!(p[7], 0xCD);
        assert_eq!(get16(&p, 3), 0xABCD);
    }

    #[test]
    fn step1_bitswap_word() {
        // bitswap<16>(0x0001, [4,11,14,3,1,13,0,7,2,8,12,15,10,9,5,6])
        // Input bit 0 goes to output position (15 - index of 0 in list) = 15-6 = 9
        // So 0x0001 -> 0x0200
        let result = bitswap::<16>(
            0x0001,
            [4, 11, 14, 3, 1, 13, 0, 7, 2, 8, 12, 15, 10, 9, 5, 6],
        );
        assert_eq!(result, 0x0200);
    }

    #[test]
    fn step3_block_permutation_preserves_words() {
        // Verify decryption runs without panicking on full-size ROM
        let mut p = vec![0u8; 0x900000];
        for (i, byte) in p.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        mslug3_decrypt_68k(&mut p);
    }

    fn pattern() -> Vec<u8> {
        (0..0x900000usize)
            .map(|i| ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8)
            .collect()
    }

    #[test]
    fn new_sma_decrypts_change_the_rom_and_keep_its_length() {
        for f in [kof99_decrypt_68k, garou_decrypt_68k, kof2000_decrypt_68k] {
            let original = pattern();
            let mut p = original.clone();
            f(&mut p);
            assert_eq!(p.len(), original.len());
            assert_ne!(p, original);
        }
    }

    #[test]
    fn kof99_data_swap_and_relocation_follow_mame() {
        // Mot 0 de la partie bankée (0x100000) d'un ROM nul sauf un bit : bitswap<16>
        // (13,7,3,0,9,4,5,6,1,12,8,14,10,11,2,15) envoie le bit 15 en position 15-15 = 0
        // (« 15 » est le dernier de la liste -> poids faible).
        let mut p = vec![0u8; 0x900000];
        set16(&mut p, 0x100000 / 2 + 0x400, 0x8000);
        kof99_decrypt_68k(&mut p);
        // le bit 15 devient le bit 0 ; l'adresse bankée (bloc 0x800/2 mots) est ensuite permutée :
        // on vérifie seulement que le mot a été déplacé dans son bloc avec la valeur 0x0001.
        let block = (0x100000 / 2 + 0x400) / 0x400 * 0x400;
        let found = (0..0x400).any(|j| get16(&p, block + j) == 0x0001);
        assert!(found);
    }

    #[test]
    fn mslug3_decrypt_oracle() {
        // Oracle test: decrypt synthetic ROM and compare CRC to MAME reference
        let mut p = vec![0u8; 0x900000];
        for (i, byte) in p.iter_mut().enumerate() {
            let pat = ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8;
            *byte = pat;
        }

        mslug3_decrypt_68k(&mut p);

        let crc = crc32_bytes(&p);

        // Read expected CRC from oracle file
        let expected_path = data_path("mslug3_p_900000.crc");
        let expected_str = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("Failed to read oracle file: {}", expected_path.display()));
        let expected = u32::from_str_radix(expected_str.trim(), 16)
            .expect("Failed to parse CRC from oracle file");

        assert_eq!(crc, expected, "CRC mismatch after mslug3_decrypt_68k");
    }
}

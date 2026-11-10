// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_pvc.cpp (mslug5_decrypt_68k, lines 100-137)
//! PVC Metal Slug 5 P-ROM decryption.

use ng_common::bitswap::bitswap;

/// XOR table for first stage of Metal Slug 5 decryption.
const XOR1: [u8; 0x20] = [
    0xc2, 0x4b, 0x74, 0xfd, 0x0b, 0x34, 0xeb, 0xd7, 0x10, 0x6d, 0xf9, 0xce, 0x5d, 0xd5, 0x61, 0x29,
    0xf5, 0xbe, 0x0d, 0x82, 0x72, 0x45, 0x0f, 0x24, 0xb3, 0x34, 0x1b, 0x99, 0xea, 0x09, 0xf3, 0x03,
];

/// XOR table for second stage of Metal Slug 5 decryption.
const XOR2: [u8; 0x20] = [
    0x36, 0x09, 0xb0, 0x64, 0x95, 0x0f, 0x90, 0x42, 0x6e, 0x0f, 0x30, 0xf6, 0xe5, 0x08, 0x30, 0x64,
    0x08, 0x04, 0x00, 0x2f, 0x72, 0x09, 0xa0, 0x13, 0xc9, 0x0b, 0xa0, 0x3e, 0xc2, 0x00, 0x40, 0x2b,
];

/// Apply XOR stages (steps 1-2) of Metal Slug 5 decryption.
pub(crate) fn xor_stage(p: &mut [u8]) {
    // Step 1: XOR first 0x100000 bytes with xor1 table
    for i in 0..0x100000 {
        p[i ^ 1] ^= XOR1[i % 0x20];
    }

    // Step 2: XOR remaining bytes with xor2 table
    for i in 0x100000..0x800000 {
        p[i ^ 1] ^= XOR2[i % 0x20];
    }
}

/// Decrypt Metal Slug 5 P-ROM data.
///
/// Port of MAME's pvc_prot_device::mslug5_decrypt_68k().
/// Operates on the full ROM image in-place. The input must be exactly
/// 0x800000 bytes (the fixed part plus one bank).
pub fn mslug5_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() == 0x800000,
        "mslug5_decrypt_68k requires exactly 0x800000 bytes"
    );

    // Steps 1-2: XOR with tables
    xor_stage(p);

    // Step 3: Bitswap words at specific locations
    for i in (0x100000..0x800000).step_by(4) {
        // Read little-endian word from MAME's rom[i+1] and rom[i+2]
        // In Rust big-endian: rom[X] -> p[X^1], so rom[i+1]->p[i], rom[i+2]->p[i+3]
        let rom16 = (p[i] as u16) | ((p[i + 3] as u16) << 8);

        // Apply bitswap<16> with the specified bit order
        let swapped = bitswap::<16>(
            rom16 as u32,
            [15, 14, 13, 12, 10, 11, 8, 9, 6, 7, 4, 5, 3, 2, 1, 0],
        ) as u16;

        // Write back: rom[i+1] = low byte, rom[i+2] = high byte
        p[i] = (swapped & 0xFF) as u8;
        p[i + 3] = (swapped >> 8) as u8;
    }

    // Step 4: Block permutation (two parts)
    let buf = p.to_vec();

    // Part A: Permute 16 blocks of 0x10000 bytes in the first 0x100000 region
    for i in 0..(0x100000 / 0x10000) {
        let ofst = (i & 0xF0) + bitswap::<8>((i & 0x0F) as u32, [7, 6, 5, 4, 1, 0, 3, 2]) as usize;
        p[i * 0x10000..(i + 1) * 0x10000]
            .copy_from_slice(&buf[ofst * 0x10000..(ofst + 1) * 0x10000]);
    }

    // Part B: Permute 0x100-byte blocks in the remaining region
    for i in (0x100000..0x800000).step_by(0x100) {
        let ofst = (i & 0xF000FF)
            + ((i & 0x000F00) ^ 0x00700)
            + (bitswap::<8>(((i & 0x0FF000) >> 12) as u32, [5, 4, 7, 6, 1, 0, 3, 2]) << 12)
                as usize;
        p[i..i + 0x100].copy_from_slice(&buf[ofst..ofst + 0x100]);
    }

    // Step 5: Final block swap
    let buf = p.to_vec();
    p[0x100000..0x200000].copy_from_slice(&buf[0x700000..0x800000]);
    p[0x200000..0x800000].copy_from_slice(&buf[0x100000..0x700000]);
}

// ---- T1010 : svc et kof2003 (ports exacts de prot_pvc.cpp) ----
//
// MAME indexe la région 68000 en mémoire hôte (octets de chaque mot échangés) : `rom[k]` de MAME
// est `p[k ^ 1]` dans notre tampon big-endian. Les copies de blocs (offsets et longueurs pairs)
// sont identiques dans les deux représentations.

const SVC_XOR1: [u8; 0x20] = [
    0x3b, 0x6a, 0xf7, 0xb7, 0xe8, 0xa9, 0x20, 0x99, 0x9f, 0x39, 0x34, 0x0c, 0xc3, 0x9a, 0xa5, 0xc8,
    0xb8, 0x18, 0xce, 0x56, 0x94, 0x44, 0xe3, 0x7a, 0xf7, 0xdd, 0x42, 0xf0, 0x18, 0x60, 0x92, 0x9f,
];
const SVC_XOR2: [u8; 0x20] = [
    0x69, 0x0b, 0x60, 0xd6, 0x4f, 0x01, 0x40, 0x1a, 0x9f, 0x0b, 0xf0, 0x75, 0x58, 0x0e, 0x60, 0xb4,
    0x14, 0x04, 0x20, 0xe4, 0xb9, 0x0d, 0x10, 0x89, 0xeb, 0x07, 0x30, 0x90, 0x50, 0x0e, 0x20, 0x26,
];
const KOF2003_XOR2: [u8; 0x20] = [
    0x2f, 0x02, 0x60, 0xbb, 0x77, 0x01, 0x30, 0x08, 0xd8, 0x01, 0xa0, 0xdf, 0x37, 0x0a, 0xf0, 0x65,
    0x28, 0x03, 0xd0, 0x23, 0xd3, 0x03, 0x70, 0x42, 0xbb, 0x06, 0xf0, 0x28, 0xba, 0x0f, 0xf0, 0x7a,
];

/// Échange de lignes de données des mots (rom[i+1] | rom[i+2] << 8), tous les 4 octets.
fn swap_word_bits(p: &mut [u8], from: usize, to: usize, order: [u8; 16]) {
    for i in (from..to).step_by(4) {
        let rom16 = (p[i] as u16) | ((p[i + 3] as u16) << 8);
        let swapped = bitswap::<16>(rom16 as u32, order) as u16;
        p[i] = swapped as u8;
        p[i + 3] = (swapped >> 8) as u8;
    }
}

/// MAME `svc_px_decrypt` (P de 0x800000 octets).
pub fn svc_px_decrypt(p: &mut [u8]) {
    assert!(
        p.len() == 0x800000,
        "svc_px_decrypt requires exactly 0x800000 bytes"
    );
    for i in 0..0x100000 {
        p[i ^ 1] ^= SVC_XOR1[i % 0x20];
    }
    for i in 0x100000..0x800000 {
        p[i ^ 1] ^= SVC_XOR2[i % 0x20];
    }
    swap_word_bits(
        p,
        0x100000,
        0x800000,
        [15, 14, 13, 12, 10, 11, 8, 9, 6, 7, 4, 5, 3, 2, 1, 0],
    );

    let buf = p.to_vec();
    for i in 0..0x100000 / 0x10000 {
        let ofst = (i & 0xF0) + bitswap::<8>((i & 0x0F) as u32, [7, 6, 5, 4, 2, 3, 0, 1]) as usize;
        p[i * 0x10000..(i + 1) * 0x10000]
            .copy_from_slice(&buf[ofst * 0x10000..(ofst + 1) * 0x10000]);
    }
    for i in (0x100000..0x800000).step_by(0x100) {
        let ofst = (i & 0xF000FF)
            + ((i & 0x000F00) ^ 0x00A00)
            + ((bitswap::<8>(((i & 0x0FF000) >> 12) as u32, [4, 5, 6, 7, 1, 0, 3, 2]) as usize)
                << 12);
        p[i..i + 0x100].copy_from_slice(&buf[ofst..ofst + 0x100]);
    }
    let buf = p.to_vec();
    p[0x100000..0x200000].copy_from_slice(&buf[0x700000..0x800000]);
    p[0x200000..0x800000].copy_from_slice(&buf[0x100000..0x700000]);
}

/// MAME `kof2003_decrypt_68k` (P de 0x900000 octets).
pub fn kof2003_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() == 0x900000,
        "kof2003_decrypt_68k requires exactly 0x900000 bytes"
    );
    for i in 0..0x100000 {
        // rom[0x800000 + i] ^= rom[0x100002 | i]
        let src = p[(0x100002 | i) ^ 1];
        p[(0x800000 + i) ^ 1] ^= src;
    }
    for i in 0..0x100000 {
        p[i ^ 1] ^= SVC_XOR1[i % 0x20];
    }
    for i in 0x100000..0x800000 {
        p[i ^ 1] ^= KOF2003_XOR2[i % 0x20];
    }
    swap_word_bits(
        p,
        0x100000,
        0x800000,
        [15, 14, 13, 12, 5, 4, 7, 6, 9, 8, 11, 10, 3, 2, 1, 0],
    );

    let mut buf = vec![0u8; 0x900000];
    for i in 0..0x100000 / 0x10000 {
        let ofst = (i & 0xF0) + bitswap::<8>((i & 0x0F) as u32, [7, 6, 5, 4, 0, 1, 2, 3]) as usize;
        buf[i * 0x10000..(i + 1) * 0x10000]
            .copy_from_slice(&p[ofst * 0x10000..(ofst + 1) * 0x10000]);
    }
    for i in (0x100000..0x900000).step_by(0x100) {
        let ofst = (i & 0xF000FF)
            + ((i & 0x000F00) ^ 0x00800)
            + ((bitswap::<8>(((i & 0x0FF000) >> 12) as u32, [4, 5, 6, 7, 1, 0, 3, 2]) as usize)
                << 12);
        buf[i..i + 0x100].copy_from_slice(&p[ofst..ofst + 0x100]);
    }
    p[0x000000..0x100000].copy_from_slice(&buf[0x000000..0x100000]);
    p[0x100000..0x200000].copy_from_slice(&buf[0x800000..0x900000]);
    p[0x200000..0x900000].copy_from_slice(&buf[0x100000..0x800000]);
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
    fn xor_stage_first_bytes() {
        let mut p = vec![0u8; 0x800000];
        xor_stage(&mut p);

        // After step 1: p[0^1] = XOR1[0] = 0xC2, so p[1] should be 0xC2
        assert_eq!(p[1], 0xC2, "p[1] should be XOR1[0]");
        // p[0] = XOR1[1] = 0x4B
        assert_eq!(p[0], 0x4B, "p[0] should be XOR1[1]");

        // After step 2: i=0x100000, p[0x100001] = XOR2[0] = 0x36
        assert_eq!(p[0x100001], 0x36, "p[0x100001] should be XOR2[0]");
    }

    #[test]
    fn svc_and_kof2003_change_the_rom_and_keep_its_length() {
        let make = |n: usize| -> Vec<u8> {
            (0..n)
                .map(|i| ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8)
                .collect()
        };
        let mut a = make(0x800000);
        let a0 = a.clone();
        svc_px_decrypt(&mut a);
        assert_eq!(a.len(), 0x800000);
        assert_ne!(a, a0);

        let mut b = make(0x900000);
        let b0 = b.clone();
        kof2003_decrypt_68k(&mut b);
        assert_eq!(b.len(), 0x900000);
        assert_ne!(b, b0);
    }

    #[test]
    fn svc_xor_of_a_zero_rom_is_visible_in_the_relocated_first_mb() {
        // ROM nulle : après les XOR/permutations, l'octet hôte rom[0] = xor1[0] = 0x3B reste
        // à l'offset 0 (la 1re partie 0x100000 est seulement permutée par blocs de 0x10000 :
        // le bloc 0 vient du bloc 0).
        let mut p = vec![0u8; 0x800000];
        svc_px_decrypt(&mut p);
        assert_eq!(p[1], 0x3B); // rom[0] hôte = p[1]
        assert_eq!(p[0], 0x6A); // rom[1] hôte = p[0] = xor1[1]
    }

    #[test]
    fn bitswap_8_test() {
        // bitswap::<8>(5, [7,6,5,4,1,0,3,2]) == 5 (verified by tracing)
        let result = bitswap::<8>(5, [7, 6, 5, 4, 1, 0, 3, 2]);
        assert_eq!(result, 5);
    }

    #[test]
    fn mslug5_decrypt_oracle() {
        // Oracle test: decrypt synthetic ROM and compare CRC to MAME reference
        let mut p = vec![0u8; 0x800000];
        for (i, byte) in p.iter_mut().enumerate() {
            let pat = ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8;
            *byte = pat;
        }

        mslug5_decrypt_68k(&mut p);

        let crc = crc32_bytes(&p);

        // Read expected CRC from oracle file
        let expected_path = data_path("mslug5_p_800000.crc");
        let expected_str = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("Failed to read oracle file: {}", expected_path.display()));
        let expected = u32::from_str_radix(expected_str.trim(), 16)
            .expect("Failed to parse CRC from oracle file");

        assert_eq!(crc, expected, "CRC mismatch after mslug5_decrypt_68k");
    }
}

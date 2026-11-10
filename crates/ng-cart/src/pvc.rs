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

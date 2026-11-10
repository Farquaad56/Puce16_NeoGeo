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

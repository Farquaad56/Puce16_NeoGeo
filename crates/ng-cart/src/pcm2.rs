// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_pcm2.cpp (lignes 35-90)
//! PCM2 protection decryption and swap for Neo Geo ROMs.

use ng_common::bitswap::bitswap;

/// Decrypt PCM2 ROM data by swapping words within blocks.
///
/// # Arguments
/// * `v` - mutable slice of bytes representing the ROM data
/// * `value` - block size parameter (8 for Metal Slug 4)
pub fn pcm2_decrypt(v: &mut [u8], value: usize) {
    let words = v.len() / 2;
    let block_words = value / 2;

    // Process in blocks of block_words u16 words
    for i in (0..words).step_by(block_words) {
        // Copy block to buffer
        let mut buf = vec![0u8; value];
        for j in 0..block_words {
            buf[2 * j] = v[2 * (i + j)];
            buf[2 * j + 1] = v[2 * (i + j) + 1];
        }

        // XOR-swap indices within block
        for j in 0..block_words {
            let src_idx = j ^ (value / 4);
            v[2 * (i + j)] = buf[2 * src_idx];
            v[2 * (i + j) + 1] = buf[2 * src_idx + 1];
        }
    }
}

/// Swap PCM2 ROM data using address scrambling.
///
/// # Arguments
/// * `v` - mutable slice of bytes representing the ROM data (must be >= 0x1000000)
/// * `value` - variant parameter (2 for Metal Slug 5)
pub fn pcm2_swap(v: &mut [u8], value: usize) {
    assert!(
        v.len() >= 0x1000000,
        "pcm2_swap requires at least 0x1000000 bytes"
    );

    // Address XOR tables from MAME prot_pcm2.cpp lines 61-76
    let addrs = [
        [0x000000u32, 0xa5000],
        [0xffce20, 0x01000],
        [0xfe2cf6, 0x4e001],
        [0xffac28, 0xc2000],
        [0xfeb2c0, 0x0a000],
        [0xff14ea, 0xa7001],
        [0xffb440, 0x02000],
    ];

    let xordata = [
        [0xf9u8, 0xe0, 0x5d, 0xf3, 0xea, 0x92, 0xbe, 0xef],
        [0xc4, 0x83, 0xa8, 0x5f, 0x21, 0x27, 0x64, 0xaf],
        [0xc3, 0xfd, 0x81, 0xac, 0x6d, 0xe7, 0xbf, 0x9e],
        [0xc3, 0xfd, 0x81, 0xac, 0x6d, 0xe7, 0xbf, 0x9e],
        [0xcb, 0x29, 0x7d, 0x43, 0xd2, 0x3a, 0xc2, 0xb4],
        [0x4b, 0xa4, 0x63, 0x46, 0xf0, 0x91, 0xea, 0x62],
        [0x4b, 0xa4, 0x63, 0x46, 0xf0, 0x91, 0xea, 0x62],
    ];

    // Copy input to buffer (like MAME's std::vector<uint8_t> buf(0x1000000))
    let mut buf = vec![0u8; 0x1000000];
    buf[..0x1000000].copy_from_slice(&v[..0x1000000]);

    // Bitswap permutation from MAME: bitswap<24>(i, 23,22,21,20,19,18,17,0,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,16)
    let bitswap_perm: [u8; 24] = [
        23, 22, 21, 20, 19, 18, 17, 0, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 16,
    ];

    for i in 0..0x1000000 {
        let j = bitswap::<24>(i as u32, bitswap_perm) as usize;
        let j_xor = (j ^ addrs[value][1] as usize) & 0xFFFFFF;
        let d = (i + addrs[value][0] as usize) & 0xFFFFFF;
        v[j_xor] = buf[d] ^ xordata[value][j_xor & 7];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ng_common::crc32_bytes;

    fn pat(i: usize) -> u8 {
        ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8
    }

    #[test]
    fn decrypt_basic() {
        // Test with value=8 on [0,1,2,3,4,5,6,7] (bytes)
        let mut v = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
        pcm2_decrypt(&mut v, 8);

        // Words: (0x0100, 0x0302, 0x0504, 0x0706) -> permuted by j^2
        // Expected: [mot2, mot3, mot0, mot1] = bytes [4,5,6,7,0,1,2,3]
        assert_eq!(v, vec![4u8, 5, 6, 7, 0, 1, 2, 3]);
    }

    #[test]
    fn swap_basic() {
        // Test with value=2 on pattern data
        let mut v = vec![0u8; 0x1000000];
        for (i, byte) in v.iter_mut().enumerate() {
            *byte = pat(i);
        }

        pcm2_swap(&mut v, 2);

        // Verify specific byte: at j=0x4E001 (after bitswap and XOR)
        // d = (0 + 0xFE2CF6) & 0xFFFFFF = 0xFE2CF6
        // buf[0xFE2CF6] ^ xordata[2][0x4E001 & 7] = pat(0xFE2CF6) ^ 0xFD
        let expected = pat(0xFE2CF6) ^ 0xFD;
        assert_eq!(v[0x4E001], expected);
    }

    #[test]
    #[should_panic(expected = "pcm2_swap requires at least 0x1000000 bytes")]
    fn swap_size_assert() {
        let mut v = vec![0u8; 0xFFFFF]; // Too small
        pcm2_swap(&mut v, 2);
    }

    #[test]
    fn decrypt_mame_oracle() {
        // MAME oracle: pcm2_decrypt(v,8) with v=0x10000 bytes of pat pattern
        let mut v = vec![0u8; 0x10000];
        for (i, byte) in v.iter_mut().enumerate() {
            *byte = pat(i);
        }

        pcm2_decrypt(&mut v, 8);

        // CRC-32/ISO-HDLC of result should match MAME oracle
        let crc = crc32_bytes(&v);
        assert_eq!(crc, 0x78E4F10Eu32);
    }

    #[test]
    fn swap_mame_oracle() {
        // MAME oracle: pcm2_swap(v,2) with v=0x1000000 bytes of pat pattern
        let mut v = vec![0u8; 0x1000000];
        for (i, byte) in v.iter_mut().enumerate() {
            *byte = pat(i);
        }

        pcm2_swap(&mut v, 2);

        // CRC-32/ISO-HDLC of result should match MAME oracle
        let crc = crc32_bytes(&v);
        assert_eq!(crc, 0xE8845307u32);
    }
}

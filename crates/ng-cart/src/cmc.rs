// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_cmc.cpp (lines 474-737)
//! Neo Geo ROM decryption for CMC42 and CMC50 protection chips.

use crate::cmc_tables::*;
use ng_common::bitswap::bitswap;

/// Lookup tables used by the decryption algorithm.
struct Tables {
    type0_t03: &'static [u8; 256],
    type0_t12: &'static [u8; 256],
    type1_t03: &'static [u8; 256],
    type1_t12: &'static [u8; 256],
    address_8_15_xor1: &'static [u8; 256],
    address_8_15_xor2: &'static [u8; 256],
    address_16_23_xor1: &'static [u8; 256],
    address_16_23_xor2: &'static [u8; 256],
    address_0_7_xor: &'static [u8; 256],
}

/// Elementary decrypt operation (MAME lines 474-498).
#[allow(clippy::too_many_arguments)]
fn decrypt(
    r0: &mut u8,
    r1: &mut u8,
    c0: u8,
    c1: u8,
    table0hi: &[u8; 256],
    table0lo: &[u8; 256],
    table1: &[u8; 256],
    address_0_7_xor: &[u8; 256],
    base: u32,
    invert: bool,
) {
    let tmp =
        table1[((base & 0xff) ^ (address_0_7_xor[((base >> 8) & 0xff) as usize] as u32)) as usize];
    let xor0 = (table0hi[((base >> 8) & 0xff) as usize] & 0xfe) | (tmp & 0x01);
    let xor1 = (tmp & 0xfe) | (table0lo[((base >> 8) & 0xff) as usize] & 0x01);

    if invert {
        *r0 = c1 ^ xor0;
        *r1 = c0 ^ xor1;
    } else {
        *r0 = c0 ^ xor0;
        *r1 = c1 ^ xor1;
    }
}

/// Full graphics ROM decryption (MAME lines 501-551).
fn gfx_decrypt(rom: &mut [u8], tables: Tables, extra_xor: i32) {
    let rom_size = rom.len();
    assert!(
        rom_size.is_power_of_two() || rom_size == 0x300_0000 || rom_size == 0x600_0000,
        "ROM size must be power of two (or 0x3000000, 0x6000000), got 0x{rom_size:X}"
    );

    // Data xor pass (MAME lines 507-511)
    let mut buf = vec![0u8; rom_size];
    for rpos in 0..(rom_size / 4) {
        let rp32 = rpos as u32;
        // Compute both decrypts into locals to avoid borrow checker issues
        let (d0, d1) = {
            let mut a = 0u8;
            let mut b = 0u8;
            decrypt(
                &mut a,
                &mut b,
                rom[4 * rpos],
                rom[4 * rpos + 3],
                tables.type0_t03,
                tables.type0_t12,
                tables.type1_t03,
                tables.address_0_7_xor,
                rp32,
                (rp32 >> 8) & 1 != 0,
            );
            (a, b)
        };
        let (d2, d3) = {
            let mut a = 0u8;
            let mut b = 0u8;
            decrypt(
                &mut a,
                &mut b,
                rom[4 * rpos + 1],
                rom[4 * rpos + 2],
                tables.type0_t12,
                tables.type0_t03,
                tables.type1_t12,
                tables.address_0_7_xor,
                rp32,
                ((rp32 >> 16) ^ (tables.address_16_23_xor2[((rp32 >> 8) & 0xff) as usize] as u32))
                    & 1
                    != 0,
            );
            (a, b)
        };
        buf[4 * rpos] = d0;
        buf[4 * rpos + 3] = d1;
        buf[4 * rpos + 1] = d2;
        buf[4 * rpos + 2] = d3;
    }

    // Address xor pass (MAME lines 514-550)
    for rpos in 0..(rom_size / 4) {
        let mut baser = rpos as u32;
        baser ^= extra_xor as u32;
        baser ^= (tables.address_8_15_xor1[((baser >> 16) & 0xff) as usize] as u32) << 8;
        baser ^= (tables.address_8_15_xor2[(baser & 0xff) as usize] as u32) << 8;
        baser ^= (tables.address_16_23_xor1[(baser & 0xff) as usize] as u32) << 16;
        baser ^= (tables.address_16_23_xor2[((baser >> 8) & 0xff) as usize] as u32) << 16;
        baser ^= tables.address_0_7_xor[((baser >> 8) & 0xff) as usize] as u32;

        // Taille réelle de la ROM (MAME lignes 529-544) : 0x3000000 (preisle2, mslug4) et
        // 0x6000000 (kf2k3pcb) ont un traitement particulier pour le dernier bloc.
        let quarter = (rom_size / 4) as u32;
        if rom_size == 0x300_0000 {
            if (rpos as u32) < 0x200_0000 / 4 {
                baser &= 0x200_0000 / 4 - 1;
            } else {
                baser = 0x200_0000 / 4 + (baser & (0x100_0000 / 4 - 1));
            }
        } else if rom_size == 0x600_0000 {
            if (rpos as u32) < 0x400_0000 / 4 {
                baser &= 0x400_0000 / 4 - 1;
            } else {
                baser = 0x400_0000 / 4 + (baser & (0x100_0000 / 4 - 1));
            }
        } else {
            baser &= quarter - 1;
        }

        rom[4 * rpos] = buf[4 * baser as usize];
        rom[4 * rpos + 1] = buf[4 * baser as usize + 1];
        rom[4 * rpos + 2] = buf[4 * baser as usize + 2];
        rom[4 * rpos + 3] = buf[4 * baser as usize + 3];
    }
}

/// CMC42 protection chip decryption (MAME lines 555-567).
pub fn cmc42_gfx_decrypt(rom: &mut [u8], extra_xor: u32) {
    let tables = Tables {
        type0_t03: &KOF99_TYPE0_T03,
        type0_t12: &KOF99_TYPE0_T12,
        type1_t03: &KOF99_TYPE1_T03,
        type1_t12: &KOF99_TYPE1_T12,
        address_8_15_xor1: &KOF99_ADDRESS_8_15_XOR1,
        address_8_15_xor2: &KOF99_ADDRESS_8_15_XOR2,
        address_16_23_xor1: &KOF99_ADDRESS_16_23_XOR1,
        address_16_23_xor2: &KOF99_ADDRESS_16_23_XOR2,
        address_0_7_xor: &KOF99_ADDRESS_0_7_XOR,
    };
    gfx_decrypt(rom, tables, extra_xor as i32);
}

/// CMC50 protection chip decryption (MAME lines 578-590).
pub fn cmc50_gfx_decrypt(rom: &mut [u8], extra_xor: u32) {
    let tables = Tables {
        type0_t03: &KOF2000_TYPE0_T03,
        type0_t12: &KOF2000_TYPE0_T12,
        type1_t03: &KOF2000_TYPE1_T03,
        type1_t12: &KOF2000_TYPE1_T12,
        address_8_15_xor1: &KOF2000_ADDRESS_8_15_XOR1,
        address_8_15_xor2: &KOF2000_ADDRESS_8_15_XOR2,
        address_16_23_xor1: &KOF2000_ADDRESS_16_23_XOR1,
        address_16_23_xor2: &KOF2000_ADDRESS_16_23_XOR2,
        address_0_7_xor: &KOF2000_ADDRESS_0_7_XOR,
    };
    gfx_decrypt(rom, tables, extra_xor as i32);
}

/// S-FIX decryption for Metal Slug games (MAME lines 604-612).
pub fn sfix_decrypt(c_rom: &[u8], fixed: &mut [u8]) {
    let tx_size = fixed.len();
    let src = &c_rom[c_rom.len() - tx_size..];

    #[allow(clippy::needless_range_loop)]
    for i in 0..tx_size {
        // MAME formula: (i & ~0x1f) + ((i & 7) << 2) + ((~i & 8) >> 2) + ((i & 0x10) >> 4)
        let idx = (i & !0x1F)
            | (((i & 7) as u32) << 2) as usize
            | ((((!i) & 8) >> 2) as u32) as usize
            | (((i & 0x10) >> 4) as u32) as usize;
        fixed[i] = src[idx];
    }
}

/// M1 address scramble for CMC50 protection (MAME lines 664-691).
fn m1_address_scramble(address: u32, key: u16) -> u32 {
    // P1 permutation table from MAME lines 666-675
    let p1 = [
        [15u8, 14, 10, 7, 1, 2, 3, 8, 0, 12, 11, 13, 6, 9, 5, 4],
        [7, 1, 8, 11, 15, 9, 2, 3, 5, 13, 4, 14, 10, 0, 6, 12],
        [8, 6, 14, 3, 10, 7, 15, 1, 4, 0, 2, 5, 13, 11, 12, 9],
        [2, 8, 15, 9, 3, 4, 11, 7, 13, 6, 0, 10, 1, 12, 14, 5],
        [1, 13, 6, 15, 14, 3, 8, 10, 9, 4, 7, 12, 5, 2, 0, 11],
        [11, 15, 3, 4, 7, 0, 9, 2, 6, 14, 12, 1, 8, 5, 10, 13],
        [10, 5, 13, 8, 6, 15, 1, 14, 11, 9, 3, 0, 12, 7, 4, 2],
        [9, 3, 7, 0, 2, 12, 4, 11, 14, 10, 5, 8, 15, 13, 1, 6],
    ];

    let block = ((address >> 16) & 7) as usize;
    let mut aux: u32 = address & 0xFFFF;

    // MAME line 680: aux ^= bitswap<16>(key, ...)
    aux ^= bitswap::<16>(
        key as u32,
        [12, 0, 2, 4, 8, 15, 7, 13, 10, 1, 3, 6, 11, 9, 14, 5],
    );

    // MAME lines 681-685: bitswap with P1[block] in REVERSE order (index 15 first)
    let row = &p1[block];
    aux = bitswap::<16>(
        aux,
        [
            row[15], row[14], row[13], row[12], row[11], row[10], row[9], row[8], row[7], row[6],
            row[5], row[4], row[3], row[2], row[1], row[0],
        ],
    );

    // MAME lines 686-687: XOR with lookup tables
    aux ^= M1_ADDRESS_0_7_XOR[((aux >> 8) & 0xFF) as usize] as u32;
    aux ^= (M1_ADDRESS_8_15_XOR[(aux & 0xFF) as usize] as u32) << 8;

    // MAME line 688: final bitswap
    aux = bitswap::<16>(aux, [7, 15, 14, 6, 5, 13, 12, 4, 11, 3, 10, 2, 9, 1, 8, 0]);

    ((block as u32) << 16) | aux
}

/// CMC50 M1 ROM decryption (MAME lines 694-737).
pub fn cmc50_m1_decrypt(m_crypt: &mut [u8], m: &mut [u8]) {
    let rom_size = 0x80000;

    // Compute key from first 64KB (MAME line 704)
    let key: u16 = m_crypt[..0x10000]
        .iter()
        .fold(0u16, |acc, &b| acc.wrapping_add(b as u16));

    // Scramble addresses (MAME lines 708-709)
    let mut buffer = vec![0u8; rom_size];
    #[allow(clippy::needless_range_loop)]
    for i in 0..rom_size {
        let scrambled = m1_address_scramble(i as u32, key);
        buffer[i] = m_crypt[scrambled as usize];
    }

    // Copy back (MAME line 711)
    m_crypt[..rom_size].copy_from_slice(&buffer);

    // Copy to audio ROM (MAME lines 713-714)
    m[..0x10000].copy_from_slice(&m_crypt[..0x10000]);
    m[0x10000..0x90000].copy_from_slice(&m_crypt[..0x80000]);
}

/// M1 address XOR table (MAME lines 623-641).
const M1_ADDRESS_8_15_XOR: [u8; 256] = [
    0x0a, 0x72, 0xb7, 0xaf, 0x67, 0xde, 0x1d, 0xb1, 0x78, 0xc4, 0x4f, 0xb5, 0x4b, 0x18, 0x76, 0xdd,
    0x11, 0xe2, 0x36, 0xa1, 0x82, 0x03, 0x98, 0xa0, 0x10, 0x5f, 0x3f, 0xd6, 0x1f, 0x90, 0x6a, 0x0b,
    0x70, 0xe0, 0x64, 0xcb, 0x9f, 0x38, 0x8b, 0x53, 0x04, 0xca, 0xf8, 0xd0, 0x07, 0x68, 0x56, 0x32,
    0xae, 0x1c, 0x2e, 0x48, 0x63, 0x92, 0x9a, 0x9c, 0x44, 0x85, 0x41, 0x40, 0x09, 0xc0, 0xc8, 0xbf,
    0xea, 0xbb, 0xf7, 0x2d, 0x99, 0x21, 0xf6, 0xba, 0x15, 0xce, 0xab, 0xb0, 0x2a, 0x60, 0xbc, 0xf1,
    0xf0, 0x9e, 0xd5, 0x97, 0xd8, 0x4e, 0x14, 0x9d, 0x42, 0x4d, 0x2c, 0x5c, 0x2b, 0xa6, 0xe1, 0xa7,
    0xef, 0x25, 0x33, 0x7a, 0xeb, 0xe7, 0x1b, 0x6d, 0x4c, 0x52, 0x26, 0x62, 0xb6, 0x35, 0xbe, 0x80,
    0x01, 0xbd, 0xfd, 0x37, 0xf9, 0x47, 0x55, 0x71, 0xb4, 0xf2, 0xff, 0x27, 0xfa, 0x23, 0xc9, 0x83,
    0x17, 0x39, 0x13, 0x0d, 0xc7, 0x86, 0x16, 0xec, 0x49, 0x6f, 0xfe, 0x34, 0x05, 0x8f, 0x00, 0xe6,
    0xa4, 0xda, 0x7b, 0xc1, 0xf3, 0xf4, 0xd9, 0x75, 0x28, 0x66, 0x87, 0xa8, 0x45, 0x6c, 0x20, 0xe9,
    0x77, 0x93, 0x7e, 0x3c, 0x1e, 0x74, 0xf5, 0x8c, 0x3e, 0x94, 0xd4, 0xc2, 0x5a, 0x06, 0x0e, 0xe8,
    0x3d, 0xa9, 0xb2, 0xe3, 0xe4, 0x22, 0xcf, 0x24, 0x8e, 0x6b, 0x8a, 0x8d, 0x84, 0x4a, 0xd2, 0x91,
    0x88, 0x79, 0x57, 0xa5, 0x0f, 0xcd, 0xb9, 0xac, 0x3b, 0xaa, 0xb3, 0xd1, 0xee, 0x31, 0x81, 0x7c,
    0xd7, 0x89, 0xd3, 0x96, 0x43, 0xc5, 0xc6, 0xc3, 0x69, 0x7f, 0x46, 0xdf, 0x30, 0x5b, 0x6e, 0xe5,
    0x08, 0x95, 0x9b, 0xfb, 0xb8, 0x58, 0x0c, 0x61, 0x50, 0x5d, 0x3a, 0xa2, 0x29, 0x12, 0xfc, 0x51,
    0x7d, 0x1a, 0x02, 0x65, 0x54, 0x5e, 0x19, 0xcc, 0xdc, 0xdb, 0x73, 0xed, 0xad, 0x59, 0x2f, 0xa3,
];

/// M1 address XOR table (MAME lines 643-661).
const M1_ADDRESS_0_7_XOR: [u8; 256] = [
    0xf4, 0xbc, 0x02, 0xf7, 0x2c, 0x3d, 0xe8, 0xd9, 0x50, 0x62, 0xec, 0xbd, 0x53, 0x73, 0x79, 0x61,
    0x00, 0x34, 0xcf, 0xa2, 0x63, 0x28, 0x90, 0xaf, 0x44, 0x3b, 0xc5, 0x8d, 0x3a, 0x46, 0x07, 0x70,
    0x66, 0xbe, 0xd8, 0x8b, 0xe9, 0xa0, 0x4b, 0x98, 0xdc, 0xdf, 0xe2, 0x16, 0x74, 0xf1, 0x37, 0xf5,
    0xb7, 0x21, 0x81, 0x01, 0x1c, 0x1b, 0x94, 0x36, 0x09, 0xa1, 0x4a, 0x91, 0x30, 0x92, 0x9b, 0x9a,
    0x29, 0xb1, 0x38, 0x4d, 0x55, 0xf2, 0x56, 0x18, 0x24, 0x47, 0x9d, 0x3f, 0x80, 0x1f, 0x22, 0xa4,
    0x11, 0x54, 0x84, 0x0d, 0x25, 0x48, 0xee, 0xc6, 0x59, 0x15, 0x03, 0x7a, 0xfd, 0x6c, 0xc3, 0x33,
    0x5b, 0xc4, 0x7b, 0x5a, 0x05, 0x7f, 0xa6, 0x40, 0xa9, 0x5d, 0x41, 0x8a, 0x96, 0x52, 0xd3, 0xf0,
    0xab, 0x72, 0x10, 0x88, 0x6f, 0x95, 0x7c, 0xa8, 0xcd, 0x9c, 0x5f, 0x32, 0xae, 0x85, 0x39, 0xac,
    0xe5, 0xd7, 0xfb, 0xd4, 0x08, 0x23, 0x19, 0x65, 0x6b, 0xa7, 0x93, 0xbb, 0x2b, 0xbf, 0xb8, 0x35,
    0xd0, 0x06, 0x26, 0x68, 0x3e, 0xdd, 0xb9, 0x69, 0x2a, 0xb2, 0xde, 0x87, 0x45, 0x58, 0xff, 0x3c,
    0x9e, 0x7d, 0xda, 0xed, 0x49, 0x8c, 0x14, 0x8e, 0x75, 0x2f, 0xe0, 0x6e, 0x78, 0x6d, 0x20, 0xd2,
    0xfa, 0x2d, 0x51, 0xcc, 0xc7, 0xe7, 0x1d, 0x27, 0x97, 0xfc, 0x31, 0xdb, 0xf8, 0x42, 0xe3, 0x99,
    0x5e, 0x83, 0x0e, 0xb4, 0x2e, 0xf6, 0xc0, 0x0c, 0x4c, 0x57, 0xb6, 0x64, 0x0a, 0x17, 0xa3, 0xc1,
    0x77, 0x12, 0xfe, 0xe6, 0x8f, 0x13, 0x71, 0xe4, 0xf9, 0xad, 0x9f, 0xce, 0xd5, 0x89, 0x7e, 0x0f,
    0xc2, 0x86, 0xf3, 0x67, 0xba, 0x60, 0x43, 0xc9, 0x04, 0xb3, 0xb0, 0x1e, 0xb5, 0xc8, 0xeb, 0xa5,
    0x76, 0xea, 0x5c, 0x82, 0x1a, 0x4f, 0xaa, 0xca, 0xe1, 0x0b, 0x4e, 0xcb, 0x6a, 0xef, 0xd1, 0xd6,
];

#[cfg(test)]
mod tests {
    use super::*;
    use ng_common::crc::crc32_bytes;
    use std::path::PathBuf;

    fn data_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests-data/prot")
            .join(name)
    }

    fn make_rom(size: usize) -> Vec<u8> {
        let mut rom = vec![0u8; size];
        for (i, byte) in rom.iter_mut().enumerate() {
            *byte = ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8;
        }
        rom
    }

    fn read_crc_file(path: &str) -> u32 {
        let content = std::fs::read_to_string(data_path(path)).expect("Failed to read CRC file");
        u32::from_str_radix(content.trim(), 16).expect("Failed to parse CRC")
    }

    #[test]
    fn cmc42_gfx_decrypt_k00() {
        let mut rom = make_rom(0x10000);
        cmc42_gfx_decrypt(&mut rom, 0x00);
        let expected = read_crc_file("cmc42_010000_k00.crc");
        assert_eq!(crc32_bytes(&rom), expected);
    }

    #[test]
    fn cmc42_gfx_decrypt_k_ad() {
        let mut rom = make_rom(0x10000);
        cmc42_gfx_decrypt(&mut rom, 0xAD);
        let expected = read_crc_file("cmc42_010000_kAD.crc");
        assert_eq!(crc32_bytes(&rom), expected);
    }

    #[test]
    fn cmc50_gfx_decrypt_k31() {
        let mut rom = make_rom(0x10000);
        cmc50_gfx_decrypt(&mut rom, 0x31);
        let expected = read_crc_file("cmc50_010000_k31.crc");
        assert_eq!(crc32_bytes(&rom), expected);
    }

    #[test]
    fn cmc50_gfx_decrypt_k19() {
        let mut rom = make_rom(0x10000);
        cmc50_gfx_decrypt(&mut rom, 0x19);
        let expected = read_crc_file("cmc50_010000_k19.crc");
        assert_eq!(crc32_bytes(&rom), expected);
    }

    #[test]
    #[should_panic(expected = "power of two")]
    fn non_power_of_two_panics() {
        let mut rom = make_rom(0x10001);
        cmc42_gfx_decrypt(&mut rom, 0x00);
    }

    // Elementary decrypt tests with null tables (all zeros)
    #[test]
    fn decrypt_no_invert() {
        let mut r0 = 0;
        let mut r1 = 0;
        let zero_table: [u8; 256] = [0; 256];
        decrypt(
            &mut r0,
            &mut r1,
            0xAA,
            0xBB,
            &zero_table,
            &zero_table,
            &zero_table,
            &zero_table,
            0,
            false,
        );
        assert_eq!(r0, 0xAA); // c0 ^ 0 = c0
        assert_eq!(r1, 0xBB); // c1 ^ 0 = c1
    }

    #[test]
    fn decrypt_invert() {
        let mut r0 = 0;
        let mut r1 = 0;
        let zero_table: [u8; 256] = [0; 256];
        decrypt(
            &mut r0,
            &mut r1,
            0xAA,
            0xBB,
            &zero_table,
            &zero_table,
            &zero_table,
            &zero_table,
            0,
            true,
        );
        assert_eq!(r0, 0xBB); // c1 ^ 0 = c1 (swapped)
        assert_eq!(r1, 0xAA); // c0 ^ 0 = c0 (swapped)
    }

    // === T701 tests: sfix_decrypt and cmc50_m1_decrypt ===

    #[test]
    fn sfix_decrypt_basic() {
        // c_rom of 0x80 bytes with values i, fixed of 0x20 bytes
        let mut c_rom = vec![0u8; 0x80];
        for (i, b) in c_rom.iter_mut().enumerate() {
            *b = i as u8;
        }
        let mut fixed = vec![0u8; 0x20];

        sfix_decrypt(&c_rom, &mut fixed);

        // src starts at offset 0x60 in c_rom (0x80 - 0x20)
        // Formula: idx = (i & !0x1F) + ((i & 7) << 2) + ((!i & 8) >> 2) + ((i & 0x10) >> 4)
        // For i=0: idx = 0 + 0 + 2 + 0 = 2 -> src[2] = c_rom[0x62] = 98
        assert_eq!(fixed[0], 98);
        // For i=1: idx = 0 + 4 + 2 + 0 = 6 -> src[6] = c_rom[0x66] = 102
        assert_eq!(fixed[1], 102);
        // For i=8: idx = 0 + 32 + 0 + 0 = 32 -> src[32] = c_rom[0x80]... wait, that's out of range
        // Actually for i=8: (8 & !0x1F)=0, ((8&7)<<2)=0, ((!8&8)>>2)=0, ((8&0x10)>>4)=0 -> idx=0
        assert_eq!(fixed[8], c_rom[0x60]); // src[0] = 96
    }

    #[test]
    fn sfix_decrypt_mame_oracle() {
        let c_rom = make_rom(0x10000);
        let mut fixed = vec![0u8; 0x1000];
        sfix_decrypt(&c_rom, &mut fixed);

        let expected = read_crc_file("sfix_010000_1000.crc");
        assert_eq!(crc32_bytes(&fixed), expected);
    }

    #[test]
    fn cmc50_m1_decrypt_key_calculation() {
        // Test key calculation: sum of first 64KB bytes mod 65536
        let mut m_crypt = vec![0u8; 0x80000];
        for (i, b) in m_crypt.iter_mut().enumerate() {
            *b = (i & 0xFF) as u8;
        }

        // Expected key: sum of bytes 0..65535 where byte[i] = i & 0xFF
        // Each value 0-255 appears 256 times in first 64KB
        // Sum = 256 * (0+1+...+255) = 256 * 32640 = 8355840
        // mod 65536 = 8355840 % 65536 = 0x8000
        let key: u16 = m_crypt[..0x10000]
            .iter()
            .fold(0u16, |acc, &b| acc.wrapping_add(b as u16));
        assert_eq!(key, 0x8000);
    }

    #[test]
    fn m1_address_scramble_is_permutation() {
        // Verify that m1_address_scramble produces unique values for all inputs
        let key = 0x1234;
        let mut seen = vec![false; 0x80000];

        for i in 0..0x80000 {
            let scrambled = m1_address_scramble(i as u32, key);
            assert!(scrambled < 0x80000, "scrambled address out of range");
            assert!(
                !seen[scrambled as usize],
                "duplicate scrambled address at {}",
                i
            );
            seen[scrambled as usize] = true;
        }

        // All addresses should be covered
        for (i, &s) in seen.iter().enumerate() {
            assert!(s, "address {} not reached", i);
        }
    }

    #[test]
    fn cmc50_m1_decrypt_mame_oracle() {
        let mut m_crypt = make_rom(0x80000);
        let mut m = vec![0u8; 0x90000];

        cmc50_m1_decrypt(&mut m_crypt, &mut m);

        // Verify both output CRCs match MAME oracles
        let expected_crypt = read_crc_file("m1_crypt_080000.crc");
        assert_eq!(crc32_bytes(&m_crypt), expected_crypt);

        let expected_audio = read_crc_file("m1_audio_090000.crc");
        assert_eq!(crc32_bytes(&m), expected_audio);
    }
}

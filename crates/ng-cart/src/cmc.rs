// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_cmc.cpp (lines 474-590)
//! Neo Geo ROM decryption for CMC42 and CMC50 protection chips.

use crate::cmc_tables::*;

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
    assert!(rom_size.is_power_of_two(), "ROM size must be power of two");

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

        // Clamp to ROM size (MAME lines 543-544)
        baser &= (rom_size / 4 - 1) as u32;

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
}

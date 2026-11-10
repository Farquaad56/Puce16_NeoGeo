// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_kof98.cpp (kof98_prot_device::decrypt_68k)
//! Déchiffrement 68000 de The King of Fighters '98 (chiffrement « précoce »).

/// Copie de 2 octets `dst[d..d+2] -> src[s..s+2]` (équivalent des `memcpy(.., 2)` de MAME).
fn copy2(dst: &mut [u8], to: usize, from: &[u8], at: usize) {
    dst[to..to + 2].copy_from_slice(&from[at..at + 2]);
}

/// Port exact de `kof98_prot_device::decrypt_68k` : permutations par blocs de 0x200 sur le
/// premier mégaoctet, puis `memmove(&p[0x100000], &p[0x200000], 0x400000)`.
/// Les offsets sont tous pairs : le tampon big-endian donne le même résultat que la mémoire hôte.
pub fn kof98_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() >= 0x600000,
        "kof98_decrypt_68k requires at least 0x600000 bytes"
    );
    const SEC: [usize; 8] = [
        0x000000, 0x100000, 0x000004, 0x100004, 0x10000A, 0x00000A, 0x10000E, 0x00000E,
    ];
    const POS: [usize; 4] = [0x000, 0x004, 0x00A, 0x00E];

    let dst = p[..0x200000].to_vec();
    let mut i = 0x800;
    while i < 0x100000 {
        for j in (0..0x100).step_by(0x10) {
            for k in (0..16).step_by(2) {
                copy2(p, i + j + k, &dst, i + j + SEC[k / 2] + 0x100);
                copy2(p, i + j + k + 0x100, &dst, i + j + SEC[k / 2]);
            }
            if (0x080000..0x0C0000).contains(&i) {
                for pos in POS {
                    copy2(p, i + j + pos, &dst, i + j + pos);
                    copy2(p, i + j + pos + 0x100, &dst, i + j + pos + 0x100);
                }
            } else if i >= 0x0C0000 {
                for pos in POS {
                    copy2(p, i + j + pos, &dst, i + j + pos + 0x100);
                    copy2(p, i + j + pos + 0x100, &dst, i + j + pos);
                }
            }
        }
        copy2(p, i, &dst, i);
        copy2(p, i + 0x2, &dst, i + 0x100000);
        copy2(p, i + 0x100, &dst, i + 0x100);
        copy2(p, i + 0x102, &dst, i + 0x100100);
        i += 0x200;
    }
    p.copy_within(0x200000..0x600000, 0x100000);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern() -> Vec<u8> {
        (0..0x600000usize).map(|i| (i / 2) as u8).collect()
    }

    #[test]
    fn first_0x800_bytes_are_untouched_and_length_is_kept() {
        let original = pattern();
        let mut p = original.clone();
        kof98_decrypt_68k(&mut p);
        assert_eq!(p.len(), 0x600000);
        assert_eq!(&p[..0x800], &original[..0x800]);
        assert_ne!(p, original);
    }

    #[test]
    fn final_memmove_moves_the_upper_banks_down() {
        let original = pattern();
        let mut p = original.clone();
        kof98_decrypt_68k(&mut p);
        // [0x100000..0x500000] = ancien [0x200000..0x600000] (non modifié par les boucles,
        // qui n'écrivent que dans les 0x100000 premiers octets).
        assert_eq!(&p[0x100000..0x100010], &original[0x200000..0x200010]);
        assert_eq!(&p[0x4FFFF0..0x500000], &original[0x5FFFF0..0x600000]);
    }

    #[test]
    fn first_block_follows_the_mame_tables() {
        // i = 0x800, j = 0, k = 0 : src[0x800..2] = dst[0x800 + sec[0] + 0x100 ..] = dst[0x900],
        // src[0x900..2] = dst[0x800]. Octets (i/2) as u8 : dst[0x900] = 0x80, dst[0x800] = 0x00.
        let mut p = pattern();
        kof98_decrypt_68k(&mut p);
        // mots (0x800, 0x802) réécrits en fin de boucle : [0x800] = dst[0x800], [0x802] = dst[0x100800]
        assert_eq!(p[0x800], 0x00);
        assert_eq!(p[0x802], (0x100800usize / 2) as u8);
        // mot 0x100 plus loin : dst[0x900] et dst[0x100900]
        assert_eq!(p[0x900], 0x80);
        assert_eq!(p[0x902], (0x100900usize / 2) as u8);
    }
}

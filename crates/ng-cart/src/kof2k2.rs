// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_kof2k2.cpp (kof2002_decrypt_68k, samsho5_decrypt_68k, samsh5sp_decrypt_68k)
//! Déchiffrements 68000 « K2K2 » : permutation de blocs de 0x80000 octets.

/// Réordonne `sec.len()` blocs de 0x80000 octets de `region` : le bloc `i` reçoit l'ancien
/// contenu situé à `sec[i]` (les offsets sont relatifs au début de `region`).
fn permute_blocks(region: &mut [u8], sec: &[usize]) {
    const BLOCK: usize = 0x80000;
    let original = region[..sec.len() * BLOCK].to_vec();
    for (i, &s) in sec.iter().enumerate() {
        region[i * BLOCK..(i + 1) * BLOCK].copy_from_slice(&original[s..s + BLOCK]);
    }
}

/// MAME `kof2002_decrypt_68k` : 8 blocs à partir de 0x100000 (0x400000 octets).
pub fn kof2002_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() >= 0x500000,
        "kof2002_decrypt_68k requires at least 0x500000 bytes"
    );
    const SEC: [usize; 8] = [
        0x100000, 0x280000, 0x300000, 0x180000, 0x000000, 0x380000, 0x200000, 0x080000,
    ];
    permute_blocks(&mut p[0x100000..], &SEC);
}

/// MAME `samsho5_decrypt_68k` : 16 blocs sur les 0x800000 octets.
pub fn samsho5_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() >= 0x800000,
        "samsho5_decrypt_68k requires at least 0x800000 bytes"
    );
    const SEC: [usize; 16] = [
        0x000000, 0x080000, 0x700000, 0x680000, 0x500000, 0x180000, 0x200000, 0x480000, 0x300000,
        0x780000, 0x600000, 0x280000, 0x100000, 0x580000, 0x400000, 0x380000,
    ];
    permute_blocks(p, &SEC);
}

/// MAME `samsh5sp_decrypt_68k` : 16 blocs sur les 0x800000 octets.
pub fn samsh5sp_decrypt_68k(p: &mut [u8]) {
    assert!(
        p.len() >= 0x800000,
        "samsh5sp_decrypt_68k requires at least 0x800000 bytes"
    );
    const SEC: [usize; 16] = [
        0x000000, 0x080000, 0x500000, 0x480000, 0x600000, 0x580000, 0x700000, 0x280000, 0x100000,
        0x680000, 0x400000, 0x780000, 0x200000, 0x380000, 0x300000, 0x180000,
    ];
    permute_blocks(p, &SEC);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM dont chaque bloc de 0x80000 octets est rempli de son numéro de bloc.
    fn blocks(n: usize) -> Vec<u8> {
        (0..n * 0x80000).map(|i| (i / 0x80000) as u8).collect()
    }

    #[test]
    fn kof2002_moves_blocks_as_mame() {
        let mut p = blocks(0xA); // 0x500000 octets
        kof2002_decrypt_68k(&mut p);
        // bloc 0 (0x000000) et bloc 1 (0x080000) inchangés : la permutation commence à 0x100000.
        assert_eq!((p[0], p[0x80000]), (0, 1));
        // src = +0x100000 (bloc 2) ; sec[0] = 0x100000 -> ancien bloc 2+2 = 4 ; sec[1] = 0x280000 -> bloc 2+5 = 7
        assert_eq!(p[0x100000], 4);
        assert_eq!(p[0x180000], 7);
        // sec[4] = 0 -> ancien bloc 2 ; sec[7] = 0x080000 -> ancien bloc 3
        assert_eq!(p[0x100000 + 4 * 0x80000], 2);
        assert_eq!(p[0x100000 + 7 * 0x80000], 3);
    }

    #[test]
    fn samsho5_and_samsh5sp_are_block_permutations() {
        for f in [samsho5_decrypt_68k, samsh5sp_decrypt_68k] {
            let mut p = blocks(16);
            f(&mut p);
            let mut seen: Vec<u8> = (0..16).map(|b| p[b * 0x80000]).collect();
            // chaque bloc est resté homogène (contenu déplacé tel quel)
            for b in 0..16 {
                assert!(p[b * 0x80000..(b + 1) * 0x80000]
                    .iter()
                    .all(|&x| x == p[b * 0x80000]));
            }
            seen.sort_unstable();
            assert_eq!(seen, (0..16).collect::<Vec<u8>>());
        }
    }

    #[test]
    fn samsho5_first_blocks_follow_the_table() {
        // sec = 0x000000, 0x080000, 0x700000, 0x680000 : blocs 0, 1, 14, 13
        let mut p = blocks(16);
        samsho5_decrypt_68k(&mut p);
        assert_eq!([p[0], p[0x80000], p[0x100000], p[0x180000]], [0, 1, 14, 13]);
        // samsh5sp : 0x000000, 0x080000, 0x500000, 0x480000 : blocs 0, 1, 10, 9
        let mut q = blocks(16);
        samsh5sp_decrypt_68k(&mut q);
        assert_eq!([q[0], q[0x80000], q[0x100000], q[0x180000]], [0, 1, 10, 9]);
    }
}

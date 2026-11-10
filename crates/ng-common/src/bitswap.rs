// license:BSD-3-Clause
// source: ref/mame-neogeo/prot_cmc.cpp (bitswap macro)
//! Bitswap utility for Neo Geo ROM decryption.

/// Generic bitswap: output bit k = input bit args[N-1-k].
pub fn bitswap<const N: usize>(v: u32, bits: [u8; N]) -> u32 {
    let mut r = 0u32;
    for (i, &b) in bits.iter().enumerate() {
        r |= ((v >> b) & 1) << (N - 1 - i);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_swap() {
        // bitswap::<16>(0x0001, [15,14,...,0]) == 0x0001 (identity)
        let bits = [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
        assert_eq!(bitswap::<16>(0x0001, bits), 0x0001);
    }

    #[test]
    fn reverse_swap() {
        // bitswap::<16>(0x8000, [0,1,...,15]) == 0x0001 (inversion)
        let bits = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        assert_eq!(bitswap::<16>(0x8000, bits), 0x0001);
    }

    #[test]
    fn known_value() {
        // bitswap::<4>(0b1010, [3,2,1,0]) = identity for this permutation
        let bits = [3, 2, 1, 0];
        assert_eq!(bitswap::<4>(0b1010, bits), 0b1010);
    }
}

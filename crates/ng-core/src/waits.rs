//! Wait states du bus 68000 (T601).
//!
//! Seuls les wait states DOCUMENTÉS sont implémentés : accès à la ROM cartouche P
//! ($000080-$0FFFFF et $200000-$2FFFFF) : +0 (MAME ne modélise pas) ; accès aux
//! ports/registres : +0. Cette carte pose le MÉCANISME ; la table est à remplir si
//! une ROM de test hardware révèle des wait states réels.

/// Wait clocks pour un cycle bus 68000 (retourne 0 partout aujourd'hui).
pub fn bus_wait(addr: u32, _write: bool) -> u32 {
    // Zones documentées : toutes à +0 (MAME ne modélise pas les wait states Neo Geo).
    // $000080-$0FFFFF : ROM cartouche P -> 0
    // $200000-$2FFFFF : PORT / ROM banquée -> 0
    // $100000-$1FFFFF : work RAM -> 0 (non documenté)
    // $3x0000 : I/O, LSPC -> 0
    // $400000-$7FFFFF : palette -> 0
    // $800000-$BFFFFF : carte mémoire -> 0 (spec dit 2 cycles mais MAME = 0)
    // $C00000-$CFFFFF : BIOS -> 0
    // $D00000-$DFFFFF : backup RAM -> 0
    let _ = addr;
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zones_return_zero() {
        // Adresses couvrant toutes les zones du bus Neo Geo.
        for &addr in &[
            0x000100, // ROM cartouche P (fixe)
            0x100000, // work RAM
            0x300000, // I/O latch
            0x3C0000, // LSPC
            0x400000, // palette
            0xC00000, // BIOS
            0xD00000, // backup RAM
        ] {
            assert_eq!(bus_wait(addr, false), 0);
            assert_eq!(bus_wait(addr, true), 0);
        }
    }
}

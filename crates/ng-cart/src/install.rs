//! T711 : chaîne de déchiffrement par jeu (MAME `decrypt_all`).
//!
//! L'ordre est celui de MAME ; la table `Protection` de `types.rs` dit quelles étapes s'appliquent.

use crate::cmc::{cmc42_gfx_decrypt, cmc50_gfx_decrypt, cmc50_m1_decrypt, sfix_decrypt};
use crate::pcm2::{pcm2_decrypt, pcm2_swap};
use crate::pvc::mslug5_decrypt_68k;
use crate::romset::Cart;
use crate::sma::{garou_decrypt_68k, kof2000_decrypt_68k, kof99_decrypt_68k, mslug3_decrypt_68k};
use crate::types::{Cmc, PDecrypt, Pcm2};

impl Cart {
    /// Déchiffre les régions de la cartouche, dans l'ordre exact de MAME :
    /// P ROM, échange PCM2 (MS5), M1, graphismes C, S fixe, déchiffrement PCM2 (MS4).
    /// Sans effet pour les jeux non protégés (mslug, mslug2, mslugx).
    pub fn decrypt_all(&mut self) {
        let prot = self.def.prot;
        match prot.p_decrypt {
            PDecrypt::None => {}
            PDecrypt::Sma3 => mslug3_decrypt_68k(&mut self.p),
            PDecrypt::Pvc5 => mslug5_decrypt_68k(&mut self.p),
            PDecrypt::Kof99 => kof99_decrypt_68k(&mut self.p),
            PDecrypt::Garou => garou_decrypt_68k(&mut self.p),
            PDecrypt::Kof2000 => kof2000_decrypt_68k(&mut self.p),
        }
        if let Pcm2::Swap(v) = prot.pcm2 {
            pcm2_swap(&mut self.v, v as usize);
        }
        if prot.m1_decrypt {
            cmc50_m1_decrypt(&mut self.m_crypt, &mut self.m);
        }
        match prot.cmc {
            Cmc::None => {}
            Cmc::C42 => cmc42_gfx_decrypt(&mut self.c, prot.gfx_key as u32),
            Cmc::C50 => cmc50_gfx_decrypt(&mut self.c, prot.gfx_key as u32),
        }
        if prot.sfix_decrypt {
            sfix_decrypt(&self.c, &mut self.s);
        }
        if let Pcm2::Decrypt(v) = prot.pcm2 {
            pcm2_decrypt(&mut self.v, v as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romset::find_def;

    fn cart(name: &str) -> Cart {
        let def = find_def(name).expect("definition");
        let s = def.sizes;
        Cart {
            def,
            p: vec![0; s.p as usize],
            s: vec![0; s.s as usize],
            m: vec![0; s.m as usize],
            m_crypt: vec![0; s.m_crypt as usize],
            v: vec![0; s.v as usize],
            vb: Vec::new(),
            c: vec![0; s.c as usize],
        }
    }

    #[test]
    fn unprotected_cart_is_left_untouched() {
        let mut c = cart("mslug");
        c.p.iter_mut().enumerate().for_each(|(i, b)| *b = i as u8);
        c.c[..16].copy_from_slice(&[7; 16]);
        let (p, cc, s) = (c.p.clone(), c.c.clone(), c.s.clone());
        c.decrypt_all();
        assert_eq!((c.p, c.c, c.s), (p, cc, s));
    }

    #[test]
    fn mslug3_chain_runs_and_keeps_region_sizes() {
        let mut c = cart("mslug3");
        let sizes = c.def.sizes;
        c.decrypt_all();
        assert_eq!(c.p.len(), sizes.p as usize);
        assert_eq!(c.s.len(), sizes.s as usize);
        assert_eq!(c.c.len(), sizes.c as usize);
    }

    #[test]
    fn mslug4_and_mslug5_chains_run() {
        for name in ["mslug4", "mslug5"] {
            let mut c = cart(name);
            c.m_crypt
                .iter_mut()
                .enumerate()
                .for_each(|(i, b)| *b = i as u8);
            c.decrypt_all();
            assert!(c.m.len() >= 0x90000, "{name}");
        }
    }
}

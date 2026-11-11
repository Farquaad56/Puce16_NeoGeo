//! T1001 : cohérence des définitions générées du catalogue.
use crate::defs::ALL_CARTS;
use crate::romset::{find_def, load_cart};
use crate::types::{Load, Region, RuntimeProt};
use std::collections::{HashMap, HashSet};
use std::fs;

const ADPCM_B: [&str; 13] = [
    "bstars", "cyberlip", "gpilots", "joyjoy", "lbowling", "maglord", "mahretsu", "minasan",
    "nam1975", "ncombat", "ridhero", "superspy", "tpgolf",
];

#[test]
fn catalogue_has_expected_size_and_unique_names() {
    assert_eq!(ALL_CARTS.len(), 169);
    let names: HashSet<_> = ALL_CARTS.iter().map(|d| d.name).collect();
    assert_eq!(names.len(), ALL_CARTS.len());
}

#[test]
fn every_part_fits_in_its_region() {
    for d in ALL_CARTS {
        let z = d.sizes;
        for p in d.parts {
            let (end, cap) = match (p.region, p.load) {
                (Region::C, Load::Byte) => (p.dest + 2 * p.size - 1, z.c),
                (Region::C, Load::Word32Swap) => (p.dest + 2 * p.size - 2, z.c),
                (Region::C, _) => (p.dest + p.size, z.c),
                (Region::P, _) => (p.dest + p.size, z.p),
                (Region::S, _) => (p.dest + p.size, z.s),
                (Region::M, _) => (p.dest + p.size, z.m),
                (Region::MCrypt, _) => (p.dest + p.size, z.m_crypt),
                (Region::V, _) => (p.dest + p.size, z.v),
                (Region::VB, _) => (p.dest + p.size, z.vb),
            };
            assert!(
                end <= cap,
                "{} : {} dépasse sa région ({end:#x} > {cap:#x})",
                d.name,
                p.file
            );
        }
    }
}

#[test]
fn kof94_definition() {
    let d = find_def("kof94").unwrap();
    let z = d.sizes;
    assert_eq!(
        (z.p, z.s, z.m, z.v, z.vb, z.c),
        (0x200000, 0x20000, 0x30000, 0x600000, 0, 0x1000000)
    );
    assert_eq!(d.m1_reload, Some((0x10000, 0x20000)));
    assert_eq!(d.parts.len(), 15);
    let p1: Vec<_> = d.parts.iter().filter(|p| p.file == "055-p1.p1").collect();
    assert_eq!(p1.len(), 2);
    assert_eq!(
        (p1[0].dest, p1[0].src_off, p1[1].dest, p1[1].src_off),
        (0x100000, 0, 0, 0x100000)
    );
    assert!(p1
        .iter()
        .all(|p| p.load == Load::WordSwap && p.crc == 0xF10A2042));
    assert_eq!(d.prot.runtime, RuntimeProt::None);
}

#[test]
fn nam1975_has_separate_adpcm_b() {
    let d = find_def("nam1975").unwrap();
    assert_eq!(
        (d.sizes.v, d.sizes.vb, d.sizes.m),
        (0x80000, 0x180000, 0x50000)
    );
    assert_eq!(d.m1_reload, Some((0x10000, 0x40000)));
    assert_eq!(d.parts.len(), 13);
    let vb: Vec<_> = d.parts.iter().filter(|p| p.region == Region::VB).collect();
    assert_eq!(vb.len(), 3);
    assert_eq!(
        vb.iter().map(|p| p.dest).collect::<Vec<_>>(),
        vec![0, 0x80000, 0x100000]
    );
    assert_eq!(
        vb.iter().map(|p| p.file).collect::<Vec<_>>(),
        vec!["001-v21.v21", "001-v22.v22", "001-v23.v23"]
    );
}

#[test]
fn irrmaze_audio_reload_and_no_bios_part() {
    let d = find_def("irrmaze").unwrap();
    assert_eq!(d.sizes.m, 0x30000);
    assert_eq!(d.m1_reload, Some((0x10000, 0x20000)));
    assert!(d.parts.iter().all(|p| p.file != "236-bios.sp1"));
}

#[test]
fn only_listed_games_have_adpcm_b() {
    for d in ALL_CARTS {
        assert_eq!(d.sizes.vb > 0, ADPCM_B.contains(&d.name), "{}", d.name);
    }
}

#[test]
fn nam1975_synthetic_load_fills_vb() {
    let d = find_def("nam1975").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut need: HashMap<&str, usize> = HashMap::new();
    for p in d.parts {
        let e = need.entry(p.file).or_insert(0);
        *e = (*e).max((p.src_off + p.size) as usize);
    }
    for (f, n) in need {
        fs::write(dir.path().join(f), vec![0u8; n]).unwrap();
    }
    let cart = load_cart(dir.path(), d, false).unwrap();
    assert_eq!(cart.vb.len(), 0x180000);
    assert_eq!(cart.v.len(), 0x80000);
}

// ---- T1005 : jeux à puce CMC42 ----
#[cfg(test)]
mod cmc42_tests {
    use crate::romset::{find_def, Cart};
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    const GAMES: [(&str, u8); 7] = [
        ("zupapa", 0xBD),
        ("ganryu", 0x07),
        ("s1945p", 0x05),
        ("preisle2", 0x9F),
        ("bangbead", 0xF8),
        ("nitd", 0xFF),
        ("sengoku3", 0xFE),
    ];

    #[test]
    fn cmc42_protection_tables() {
        for (name, key) in GAMES {
            let p = find_def(name).expect(name).prot;
            assert_eq!(p.cmc, Cmc::C42, "{name}");
            assert_eq!(p.gfx_key, key, "{name}");
            assert!(p.sfix_decrypt, "{name}");
            assert_eq!(p.fix_bank, FixBank::Garou, "{name}");
            assert_eq!(p.runtime, RuntimeProt::None, "{name}");
            assert_eq!(p.p_decrypt, PDecrypt::None, "{name}");
            assert!(!p.m1_decrypt, "{name}");
            assert_eq!(p.pcm2, Pcm2::None, "{name}");
        }
    }

    #[test]
    fn cmc42_decrypt_chain_keeps_sizes() {
        for (name, _) in GAMES {
            let def = find_def(name).expect(name);
            let s = def.sizes;
            // MAME : région « fixed » de 0x20000 octets (les ROM S viennent des C).
            assert_eq!(s.s, 0x20000, "{name}");
            let mut c = Cart {
                def,
                p: vec![0; s.p as usize],
                s: vec![0; s.s as usize],
                m: vec![0; s.m as usize],
                m_crypt: vec![0; s.m_crypt as usize],
                v: vec![0; s.v as usize],
                vb: Vec::new(),
                c: vec![0; s.c as usize],
            };
            c.decrypt_all();
            assert_eq!(c.p.len(), s.p as usize, "{name}");
            assert_eq!(c.s.len(), s.s as usize, "{name}");
            assert_eq!(c.c.len(), s.c as usize, "{name}");
        }
    }
}

// ---- T1006 : jeux à puce CMC50 ----
#[cfg(test)]
mod cmc50_tests {
    use crate::romset::{find_def, Cart};
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn cmc50_protection_tables() {
        for (name, key) in [("kof2001", 0x1Eu8), ("jockeygp", 0xAC)] {
            let p = find_def(name).expect(name).prot;
            assert_eq!(p.cmc, Cmc::C50, "{name}");
            assert_eq!(p.gfx_key, key, "{name}");
            assert!(p.m1_decrypt && p.sfix_decrypt, "{name}");
            assert_eq!(p.fix_bank, FixBank::Garou, "{name}");
            assert_eq!(p.pcm2, Pcm2::None, "{name}");
            assert_eq!(p.p_decrypt, PDecrypt::None, "{name}");
            // T1014 : jockeygp a en plus la RAM de cartouche.
            let want = if name == "jockeygp" {
                RuntimeProt::CartRam
            } else {
                RuntimeProt::None
            };
            assert_eq!(p.runtime, want, "{name}");
        }
    }

    #[test]
    fn cmc50_decrypt_chain_runs() {
        for name in ["kof2001", "jockeygp"] {
            let def = find_def(name).expect(name);
            let s = def.sizes;
            assert_eq!((s.m, s.m_crypt), (0x90000, 0x80000), "{name}");
            let mut c = Cart {
                def,
                p: vec![0; s.p as usize],
                s: vec![0; s.s as usize],
                m: vec![0; s.m as usize],
                m_crypt: vec![0; s.m_crypt as usize],
                v: vec![0; s.v as usize],
                vb: Vec::new(),
                c: vec![0; s.c as usize],
            };
            c.decrypt_all();
            assert_eq!(c.m.len(), 0x90000, "{name}");
            assert_eq!(c.s.len(), s.s as usize, "{name}");
        }
    }
}

// ---- T1007 : jeux à puce SMA ----
#[cfg(test)]
mod sma_tests {
    use crate::romset::find_def;
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn sma_protection_tables() {
        let k99 = find_def("kof99").unwrap().prot;
        assert_eq!(k99.p_decrypt, PDecrypt::Kof99);
        assert_eq!((k99.cmc, k99.gfx_key), (Cmc::C42, 0x00));
        assert_eq!(k99.runtime, RuntimeProt::Sma99);
        assert_eq!(k99.fix_bank, FixBank::Garou);
        assert!(k99.sfix_decrypt && !k99.m1_decrypt);

        let g = find_def("garou").unwrap().prot;
        assert_eq!(g.p_decrypt, PDecrypt::Garou);
        assert_eq!((g.cmc, g.gfx_key), (Cmc::C42, 0x06));
        assert_eq!(g.runtime, RuntimeProt::SmaGarou);
        assert_eq!(g.fix_bank, FixBank::Garou);
        assert!(g.sfix_decrypt && !g.m1_decrypt);

        let k2 = find_def("kof2000").unwrap().prot;
        assert_eq!(k2.p_decrypt, PDecrypt::Kof2000);
        assert_eq!((k2.cmc, k2.gfx_key), (Cmc::C50, 0x00));
        assert!(k2.m1_decrypt && k2.sfix_decrypt);
        assert_eq!(k2.runtime, RuntimeProt::Sma2000);
        assert_eq!(k2.fix_bank, FixBank::Kof2000);

        for n in ["kof99", "garou", "kof2000"] {
            assert_eq!(find_def(n).unwrap().prot.pcm2, Pcm2::None, "{n}");
        }
    }
}

// ---- T1008 : CMC50 + brouillage PCM2 (rotd, pnyaa) ----
#[cfg(test)]
mod pcm2_game_tests {
    use crate::pcm2::pcm2_decrypt;
    use crate::romset::{find_def, Cart};
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn pcm2_protection_tables() {
        for (name, key, n) in [("rotd", 0x3Fu8, 16u8), ("pnyaa", 0x2E, 4)] {
            let p = find_def(name).expect(name).prot;
            assert_eq!(p.cmc, Cmc::C50, "{name}");
            assert_eq!(p.gfx_key, key, "{name}");
            assert_eq!(p.pcm2, Pcm2::Decrypt(n), "{name}");
            assert!(p.m1_decrypt && p.sfix_decrypt, "{name}");
            assert_eq!(p.fix_bank, FixBank::Garou, "{name}");
            assert_eq!(p.p_decrypt, PDecrypt::None, "{name}");
            assert_eq!(p.runtime, RuntimeProt::None, "{name}");
        }
    }

    #[test]
    fn pcm2_decrypt_values_16_and_4() {
        for n in [16usize, 4] {
            let mut zeros = vec![0u8; 0x1000];
            pcm2_decrypt(&mut zeros, n);
            assert_eq!(zeros.len(), 0x1000);
            let original: Vec<u8> = (0..0x1000usize).map(|i| i as u8).collect();
            let mut v = original.clone();
            pcm2_decrypt(&mut v, n);
            assert_ne!(v, original, "n = {n}");
        }
        // n = 4 : mots (a b)(c d) -> blocs de 2 mots, j ^ 1 : [0,1,2,3] -> [2,3,0,1]
        let mut v = vec![0u8, 1, 2, 3];
        pcm2_decrypt(&mut v, 4);
        assert_eq!(v, vec![2, 3, 0, 1]);
    }

    #[test]
    fn pcm2_decrypt_chain_runs() {
        for name in ["rotd", "pnyaa"] {
            let def = find_def(name).expect(name);
            let s = def.sizes;
            let mut c = Cart {
                def,
                p: vec![0; s.p as usize],
                s: vec![0; s.s as usize],
                m: vec![0; s.m as usize],
                m_crypt: vec![0; s.m_crypt as usize],
                v: vec![0; s.v as usize],
                vb: Vec::new(),
                c: vec![0; s.c as usize],
            };
            c.decrypt_all();
            assert_eq!(c.v.len(), s.v as usize, "{name}");
            assert_eq!(c.m.len(), 0x90000, "{name}");
        }
    }
}

// ---- T1009 : famille K2K2 (kof2002, samsho5, samsh5sp) ----
#[cfg(test)]
mod k2k2_tests {
    use crate::pcm2::pcm2_swap;
    use crate::romset::{find_def, Cart};
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn k2k2_protection_tables() {
        for (name, pd, key, swap, bank) in [
            ("kof2002", PDecrypt::Kof2002, 0xECu8, 0u8, FixBank::Std),
            ("samsho5", PDecrypt::Samsho5, 0x0F, 4, FixBank::Garou),
            ("samsh5sp", PDecrypt::Samsh5sp, 0x0D, 6, FixBank::Garou),
        ] {
            let p = find_def(name).expect(name).prot;
            assert_eq!(p.p_decrypt, pd, "{name}");
            assert_eq!((p.cmc, p.gfx_key), (Cmc::C50, key), "{name}");
            assert_eq!(p.pcm2, Pcm2::Swap(swap), "{name}");
            assert_eq!(p.fix_bank, bank, "{name}");
            assert!(p.m1_decrypt && p.sfix_decrypt, "{name}");
            assert_eq!(p.runtime, RuntimeProt::None, "{name}");
        }
    }

    #[test]
    fn pcm2_swap_values_0_4_6() {
        for n in [0usize, 4, 6] {
            let original: Vec<u8> = (0..0x1000000usize).map(|i| i as u8).collect();
            let mut v = original.clone();
            pcm2_swap(&mut v, n);
            assert_eq!(v.len(), original.len(), "n = {n}");
            assert_ne!(v, original, "n = {n}");
            // Appliqué deux fois : ce n'est pas une involution (décalage d'adresse + XOR de données).
            pcm2_swap(&mut v, n);
            assert_ne!(v, original, "n = {n}");
        }
    }

    #[test]
    fn k2k2_decrypt_chain_runs() {
        for name in ["kof2002", "samsho5", "samsh5sp"] {
            let def = find_def(name).expect(name);
            let s = def.sizes;
            let mut c = Cart {
                def,
                p: (0..s.p as usize)
                    .map(|i| (i >> 19) as u8 ^ i as u8)
                    .collect(),
                s: vec![0; s.s as usize],
                m: vec![0; s.m as usize],
                m_crypt: vec![0; s.m_crypt as usize],
                v: vec![0; s.v as usize],
                vb: Vec::new(),
                c: vec![0; s.c as usize],
            };
            let before = c.p.clone();
            c.decrypt_all();
            assert_eq!(c.p.len(), s.p as usize, "{name}");
            assert_ne!(c.p, before, "{name}");
            assert_eq!(c.v.len(), s.v as usize, "{name}");
        }
    }
}

// ---- T1010 : puce PVC (kof2003, svc) ----
#[cfg(test)]
mod pvc_game_tests {
    use crate::romset::{find_def, Cart};
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn pvc_protection_tables() {
        for (name, pd, key, swap) in [
            ("kof2003", PDecrypt::Kof2003, 0x9Du8, 5u8),
            ("svc", PDecrypt::Svc, 0x57, 3),
        ] {
            let p = find_def(name).expect(name).prot;
            assert_eq!(p.p_decrypt, pd, "{name}");
            assert_eq!((p.cmc, p.gfx_key), (Cmc::C50, key), "{name}");
            assert_eq!(p.pcm2, Pcm2::Swap(swap), "{name}");
            assert_eq!(p.runtime, RuntimeProt::Pvc, "{name}");
            assert_eq!(p.fix_bank, FixBank::Kof2000, "{name}");
            assert!(p.m1_decrypt && p.sfix_decrypt, "{name}");
        }
        assert_eq!(find_def("kof2003").unwrap().sizes.p, 0x900000);
        assert_eq!(find_def("svc").unwrap().sizes.p, 0x800000);
    }

    #[test]
    fn pvc_decrypt_chain_runs() {
        for name in ["kof2003", "svc"] {
            let def = find_def(name).expect(name);
            let s = def.sizes;
            let mut c = Cart {
                def,
                p: (0..s.p as usize)
                    .map(|i| (i >> 8) as u8 ^ i as u8)
                    .collect(),
                s: vec![0; s.s as usize],
                m: vec![0; s.m as usize],
                m_crypt: vec![0; s.m_crypt as usize],
                v: vec![0; s.v as usize],
                vb: Vec::new(),
                c: vec![0; s.c as usize],
            };
            let before = c.p.clone();
            c.decrypt_all();
            assert_eq!(c.p.len(), s.p as usize, "{name}");
            assert_ne!(c.p, before, "{name}");
        }
    }
}

// ---- T1011 : kof98 ----
#[cfg(test)]
mod kof98_tests {
    use crate::romset::find_def;
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn kof98_protection_table() {
        let d = find_def("kof98").unwrap();
        let p = d.prot;
        assert_eq!(p.p_decrypt, PDecrypt::Kof98);
        assert_eq!(p.runtime, RuntimeProt::Kof98);
        assert_eq!(p.cmc, Cmc::None);
        assert_eq!(p.pcm2, Pcm2::None);
        assert_eq!(p.fix_bank, FixBank::Std);
        assert!(!p.sfix_decrypt && !p.m1_decrypt);
        assert_eq!(d.sizes.p, 0x600000);
    }
}

// ---- T1012 : fatfury2 / ssideki ----
#[cfg(test)]
mod fatfury2_tests {
    use crate::romset::find_def;
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn fatfury2_ssideki_protection_table() {
        for g in ["fatfury2", "ssideki"] {
            let d = find_def(g).unwrap();
            let p = d.prot;
            assert_eq!(p.runtime, RuntimeProt::FatFury2, "{g}");
            assert_eq!(p.p_decrypt, PDecrypt::None, "{g}");
            assert_eq!(p.cmc, Cmc::None, "{g}");
            assert_eq!(p.pcm2, Pcm2::None, "{g}");
            assert_eq!(p.fix_bank, FixBank::Std, "{g}");
            assert!(!p.sfix_decrypt && !p.m1_decrypt, "{g}");
        }
        assert_eq!(find_def("fatfury2").unwrap().sizes.p, 0x100000);
        assert_eq!(find_def("ssideki").unwrap().sizes.p, 0x100000);
    }
}

// ---- T1013 : sbp ----
#[cfg(test)]
mod sbp_game_tests {
    use crate::romset::find_def;
    use crate::types::{Cmc, FixBank, PDecrypt, Pcm2, RuntimeProt};

    #[test]
    fn sbp_protection_table() {
        let d = find_def("sbp").unwrap();
        let p = d.prot;
        assert_eq!(p.p_decrypt, PDecrypt::Sbp);
        assert_eq!(p.runtime, RuntimeProt::Sbp);
        assert_eq!(p.cmc, Cmc::None);
        assert_eq!(p.pcm2, Pcm2::None);
        assert_eq!(p.fix_bank, FixBank::Std);
        assert_eq!(d.sizes.p, 0x100000);
    }
}

// ---- T1014 : RAM de cartouche ----
#[cfg(test)]
mod cartram_game_tests {
    use crate::romset::find_def;
    use crate::types::{Cmc, RuntimeProt};

    #[test]
    fn vliner_jockeygp_have_cart_ram() {
        assert_eq!(
            find_def("vliner").unwrap().prot.runtime,
            RuntimeProt::Vliner
        );
        let j = find_def("jockeygp").unwrap().prot;
        assert_eq!(j.runtime, RuntimeProt::CartRam);
        assert_eq!(j.cmc, Cmc::C50);
        assert_eq!(find_def("kof94").unwrap().prot.runtime, RuntimeProt::None);
    }
}

// ---- T1015 : mahjong ----
#[cfg(test)]
mod mahjong_game_tests {
    use crate::romset::find_def;
    use crate::types::Controller;

    #[test]
    fn mahjong_controller_table() {
        for g in ["bakatono", "janshin", "minasan"] {
            assert_eq!(find_def(g).unwrap().controller, Controller::Mahjong, "{g}");
        }
        assert_eq!(find_def("kof94").unwrap().controller, Controller::Joystick);
        assert_eq!(
            find_def("mahretsu").unwrap().controller,
            Controller::Joystick
        );
    }
}

// ---- T1016 : trackball / molette ----
#[cfg(test)]
mod pointer_game_tests {
    use crate::romset::find_def;
    use crate::types::Controller;

    #[test]
    fn trackball_and_dial_table() {
        assert_eq!(
            find_def("irrmaze").unwrap().controller,
            Controller::Trackball
        );
        assert_eq!(find_def("popbounc").unwrap().controller, Controller::Dial);
        assert_eq!(find_def("kof94").unwrap().controller, Controller::Joystick);
    }
}

// ---- T1017 : jeux hors MAME ----
#[cfg(test)]
mod extra_games_tests {
    use crate::romset::find_def;
    use crate::types::{Region, RuntimeProt};

    const EXTRA: [&str; 11] = [
        "cnbe", "columnsn", "ffeast", "ltorb", "neonopon", "neopong", "neothund", "ngem2k",
        "ngtetris", "poknight", "totc",
    ];

    #[test]
    fn extra_games_fit_their_regions() {
        for g in EXTRA {
            let d = find_def(g).unwrap_or_else(|| panic!("{g}"));
            assert!(d.sizes.c > 0, "{g}");
            assert_eq!(d.prot.runtime, RuntimeProt::None, "{g}");
            for p in d.parts {
                let cap = match p.region {
                    Region::P => d.sizes.p,
                    Region::S => d.sizes.s,
                    Region::M => d.sizes.m,
                    Region::V => d.sizes.v,
                    Region::C => d.sizes.c,
                    _ => usize::MAX as u32,
                };
                // octets pairs/impairs : le dernier octet écrit est à dest + 2 * (size - 1)
                let end = if matches!(p.region, Region::C) {
                    p.dest + 2 * p.size - 1
                } else {
                    p.dest + p.size
                };
                assert!(end <= cap, "{g} {}", p.file);
            }
        }
    }

    #[test]
    fn ngem2k_parts() {
        let d = find_def("ngem2k").unwrap();
        let files: Vec<(&str, Region, u32)> =
            d.parts.iter().map(|p| (p.file, p.region, p.dest)).collect();
        assert!(files.contains(&("411.p1", Region::P, 0)));
        assert!(files.contains(&("411.s1", Region::S, 0)));
        assert!(files.contains(&("411.m1", Region::M, 0)));
        assert!(files.contains(&("411.v1", Region::V, 0)));
        assert!(files.contains(&("411.c1", Region::C, 0)));
        assert!(files.contains(&("411.c2", Region::C, 1)));
        assert_eq!(d.sizes.m, 0x30000);
    }
}

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
    assert_eq!(ALL_CARTS.len(), 134);
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

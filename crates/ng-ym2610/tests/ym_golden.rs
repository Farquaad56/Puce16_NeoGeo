//! Compare la sortie du YM2610 aux vecteurs générés par le vrai ymfm (tests-data/ym/*.bin,
//! pilote tools/ymfm_golden.cpp). Les fichiers ne sont pas optionnels : leur absence fait échouer le test.

mod common;

use common::{run, W};
use std::path::PathBuf;

fn data_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests-data/ym")
        .join(name)
}

/// Écriture dans un registre de la banque basse (offsets 0/1).
fn lo(w: &mut Vec<W>, before: u32, reg: u8, val: u8) {
    w.push(W {
        before,
        off: 0,
        val: reg,
    });
    w.push(W {
        before,
        off: 1,
        val,
    });
}

/// Écriture dans un registre de la banque haute (offsets 2/3).
fn hi(w: &mut Vec<W>, before: u32, reg: u8, val: u8) {
    w.push(W {
        before,
        off: 2,
        val: reg,
    });
    w.push(W {
        before,
        off: 3,
        val,
    });
}

fn expected(name: &str) -> Vec<[i16; 3]> {
    let bytes = std::fs::read(data_path(&format!("{name}.bin")))
        .unwrap_or_else(|e| panic!("{name}.bin illisible : {e}"));
    assert_eq!(bytes.len(), 6000, "{name}.bin : taille inattendue");
    let (chunks, rest) = bytes.as_chunks::<6>();
    assert!(rest.is_empty());
    chunks
        .iter()
        .map(|c| {
            [
                i16::from_le_bytes([c[0], c[1]]),
                i16::from_le_bytes([c[2], c[3]]),
                i16::from_le_bytes([c[4], c[5]]),
            ]
        })
        .collect()
}

fn check(name: &str, w: &[W]) -> Vec<u8> {
    let (got, status) = run(w);
    let want = expected(name);
    assert_eq!(got.len(), want.len());
    for (i, (g, e)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g, e, "scenario {name} : premier ecart a l'echantillon {i}");
    }
    status
}

fn fm_ops(w: &mut Vec<W>, tl: u8) {
    for op in 0..4u8 {
        let o = op * 4 + 1;
        lo(w, 1, 0x30 + o, 0x01);
        lo(w, 1, 0x40 + o, tl);
        lo(w, 1, 0x50 + o, 0x1F);
        lo(w, 1, 0x60 + o, 0x05);
        lo(w, 1, 0x70 + o, 0x02);
        lo(w, 1, 0x80 + o, 0x24);
    }
}

#[test]
fn silence() {
    let want = expected("silence");
    assert!(want.iter().all(|s| *s == [0, 0, 0]));
    check("silence", &[]);
}

#[test]
fn fm() {
    let mut w = Vec::new();
    lo(&mut w, 1, 0xB1, 0x07);
    lo(&mut w, 1, 0xB5, 0xC0);
    fm_ops(&mut w, 0x10);
    lo(&mut w, 1, 0xA5, 0x22);
    lo(&mut w, 1, 0xA1, 0x69);
    lo(&mut w, 2, 0x28, 0xF1);
    check("fm", &w);
}

#[test]
fn ssg() {
    let mut w = Vec::new();
    lo(&mut w, 1, 0, 0x10);
    lo(&mut w, 1, 1, 0);
    lo(&mut w, 1, 7, 0x3E);
    lo(&mut w, 1, 8, 0x0F);
    check("ssg", &w);
}

#[test]
fn adpcma() {
    let mut w = Vec::new();
    hi(&mut w, 1, 0x01, 0x3F);
    hi(&mut w, 1, 0x08, 0xDF);
    hi(&mut w, 1, 0x10, 0);
    hi(&mut w, 1, 0x18, 0);
    hi(&mut w, 1, 0x20, 0x02);
    hi(&mut w, 1, 0x28, 0);
    hi(&mut w, 2, 0x00, 0x01);
    check("adpcma", &w);
}

#[test]
fn adpcmb() {
    let mut w = Vec::new();
    lo(&mut w, 1, 0x1B, 0xFF);
    lo(&mut w, 1, 0x11, 0xC0);
    lo(&mut w, 1, 0x12, 0);
    lo(&mut w, 1, 0x13, 0);
    lo(&mut w, 1, 0x14, 1);
    lo(&mut w, 1, 0x15, 0);
    lo(&mut w, 1, 0x19, 0);
    lo(&mut w, 1, 0x1A, 0x40);
    lo(&mut w, 1, 0x10, 0x01);
    lo(&mut w, 2, 0x10, 0x80);
    check("adpcmb", &w);
}

#[test]
fn timers() {
    let mut w = Vec::new();
    lo(&mut w, 1, 0x24, 0xFC);
    lo(&mut w, 1, 0x25, 0x00);
    lo(&mut w, 1, 0x26, 0xF0);
    lo(&mut w, 1, 0x27, 0x3F);
    lo(&mut w, 400, 0x27, 0x15);
    let status = check("timers", &w);
    let want = std::fs::read(data_path("timers_status.bin")).expect("timers_status.bin illisible");
    assert_eq!(want.len(), 1000);
    for (i, (g, e)) in status.iter().zip(want.iter()).enumerate() {
        assert_eq!(
            g, e,
            "scenario timers (statut) : premier ecart a l'echantillon {i}"
        );
    }
}

#[test]
fn mix() {
    let mut w = Vec::new();
    lo(&mut w, 1, 0xB1, 0x07);
    lo(&mut w, 1, 0xB5, 0xC0);
    fm_ops(&mut w, 0x20);
    lo(&mut w, 1, 0xA5, 0x22);
    lo(&mut w, 1, 0xA1, 0x69);
    lo(&mut w, 2, 0x28, 0xF1);
    lo(&mut w, 1, 0, 0x20);
    lo(&mut w, 1, 1, 0);
    lo(&mut w, 1, 7, 0x3E);
    lo(&mut w, 1, 8, 0x0A);
    hi(&mut w, 1, 0x01, 0x3F);
    hi(&mut w, 1, 0x08, 0xDF);
    hi(&mut w, 1, 0x10, 0);
    hi(&mut w, 1, 0x18, 0);
    hi(&mut w, 1, 0x20, 0x02);
    hi(&mut w, 1, 0x28, 0);
    hi(&mut w, 3, 0x00, 0x01);
    check("mix", &w);
}

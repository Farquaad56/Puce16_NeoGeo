// T900b - le rattrapage paresseux ne change aucune trame de Metal Slug.
// Test #[ignore] : nécessite mslug.zip et le BIOS locaux (neogeo.toml).

use ng_cart::load_config;
use ng_common::crc32_bytes;
use ng_core::load_machine;

fn crc(pixels: &[u32]) -> u32 {
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    crc32_bytes(&bytes)
}

#[test]
#[ignore = "requires local mslug.zip and BIOS"]
fn lazy_and_eager_agree_on_mslug() {
    let mut dir = std::env::current_dir().expect("cwd");
    while !dir.join("neogeo.toml").exists() {
        assert!(dir.pop(), "neogeo.toml introuvable");
    }
    let cfg = load_config(&dir.join("neogeo.toml")).expect("config");
    let mut eager = load_machine(&cfg, "mslug").expect("mslug");
    eager.lazy_catch_up = false;
    let mut lazy = load_machine(&cfg, "mslug").expect("mslug");
    for frame in 1..=600u32 {
        eager.run_frame();
        lazy.run_frame();
        if [60, 120, 600].contains(&frame) {
            assert_eq!(
                crc(&eager.framebuffer().pixels),
                crc(&lazy.framebuffer().pixels),
                "trame {frame}"
            );
        }
    }
    let (a, b) = (eager.save_state(), lazy.save_state());
    let first = a.iter().zip(&b).position(|(x, y)| x != y);
    let diffs: Vec<usize> = a
        .iter()
        .zip(&b)
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .take(12)
        .collect();
    assert!(
        a == b,
        "states differ, first byte {first:?}, offsets {diffs:?}, lens {} {}",
        a.len(),
        b.len()
    );
    eprintln!(
        "catch_ups : eager={} lazy={}",
        eager.stats.catch_ups, lazy.stats.catch_ups
    );
}

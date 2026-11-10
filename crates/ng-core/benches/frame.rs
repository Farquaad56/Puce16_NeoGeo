//! Banc de `run_frame` sur Metal Slug (BIOS et jeu locaux ; ignoré si absents).

use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, Criterion};
use ng_cart::load_config;
use ng_core::load_machine;

fn repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..5 {
        if dir.join("neogeo.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn bench_frame(c: &mut Criterion) {
    let Some(root) = repo_root() else {
        eprintln!("neogeo.toml introuvable : banc ignoré");
        return;
    };
    let Ok(cfg) = load_config(&root.join("neogeo.toml")) else {
        eprintln!("configuration illisible : banc ignoré");
        return;
    };
    let Ok(mut m) = load_machine(&cfg, "mslug") else {
        eprintln!("mslug.zip ou BIOS absents : banc ignoré");
        return;
    };
    // démarrage hors mesure
    for _ in 0..120 {
        m.run_frame();
    }
    let mut group = c.benchmark_group("run_frame");
    group.sample_size(10);
    group.bench_function("mslug_300_frames", |b| {
        b.iter(|| {
            for _ in 0..300 {
                m.run_frame();
            }
            m.take_audio().len()
        })
    });
    group.finish();
    let s = m.stats;
    eprintln!(
        "stats cumulées : catch_ups={} z80_ticks={} trames={}",
        s.catch_ups,
        s.z80_ticks,
        m.frame_count()
    );
}

criterion_group!(benches, bench_frame);
criterion_main!(benches);

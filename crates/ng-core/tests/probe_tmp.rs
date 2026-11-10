use ng_cart::load_config;
use ng_core::load_machine;
#[test]
#[ignore]
fn probe() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cfg = load_config(&root.join("neogeo.toml")).unwrap();
    cfg.bios = "sp-s2.sp1".into();
    for g in ["kof99", "kof2000", "mslug4"] {
        let mut m = load_machine(&cfg, g).unwrap();
        println!("== {g}");
        for f in 1..=700u32 {
            m.run_frame();
            if f % 100 == 0 {
                println!(
                    "f{f} z80pc={:04X} reply={:02X} z80ticks={}",
                    m.z80.regs.pc, m.sound.reply, m.stats.z80_ticks
                );
            }
        }
    }
}

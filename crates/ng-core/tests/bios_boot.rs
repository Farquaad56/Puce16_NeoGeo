// T602 — Démarrage du BIOS jusqu'à l'écran (test d'intégration)
// Charge la machine SANS cartouche (BIOS seul), exécute 180 trames, vérifie :
// - PC dans [0xC00000, 0xC1FFFF] ou RAM [0x100000, 0x10FFFF]
// - Aucune panique
// - Framebuffer contient ≥ 2 couleurs distinctes

use std::collections::HashSet;
use std::path::PathBuf;

use ng_cart::{load_bios, load_config};
use ng_core::Machine;

/// Chemin de la configuration (neogeo.toml à la racine du repo).
fn config_path() -> PathBuf {
    // Cherche neogeo.toml dans le répertoire courant ou un parent.
    let mut dir = std::env::current_dir().expect("current dir");
    for _ in 0..5 {
        let candidate = dir.join("neogeo.toml");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("neogeo.toml not found (run from repo root or set HERMES_KANBAN_WORKSPACE)");
}

/// Compte le nombre de couleurs distinctes dans le framebuffer.
fn count_colors(fb: &ng_common::Framebuffer) -> usize {
    let mut colors = HashSet::new();
    for &px in &fb.pixels {
        colors.insert(px);
    }
    colors.len()
}

/// Vérifie que l'adresse PC est dans une zone valide (BIOS ou RAM).
fn pc_in_valid_range(pc: u32) -> bool {
    // BIOS : 0xC00000..0xC1FFFF (128 Ko)
    let in_bios = (0xC00000..=0xC1FFFF).contains(&pc);
    // RAM : 0x100000..0x10FFFF (64 Ko work RAM)
    let in_ram = (0x100000..=0x10FFFF).contains(&pc);
    in_bios || in_ram
}

/// Test jalon : machine BIOS seul 180 trames sans panique.
/// #[ignore] tant que les fichiers locaux manquent (BIOS non versionné).
#[test]
#[ignore = "requires local BIOS files from neogeo.toml"]
fn bios_boot_milestone() {
    let cfg = load_config(&config_path()).expect("load neogeo.toml");

    // Charge le BIOS seul (sans cartouche).
    let bios = load_bios(&cfg.bios_dir, &cfg.bios).expect("load BIOS");

    // Construit la machine avec une cartouche vide (BIOS seul).
    let cart = ng_cart::Cart {
        def: ng_cart::find_def("mslug").expect("definition mslug"),
        p: vec![0x12; 0x1000],
        s: vec![0; 0x100],
        m: vec![0x34; 0x2000],
        m_crypt: Vec::new(),
        v: vec![0x56; 0x400],
        c: vec![0; 0x100],
    };

    let mut m = Machine::new(cart, bios, cfg.model.clone());

    // Exécute 180 trames (~3 secondes émulées à ~60 fps).
    for _ in 0..180 {
        m.run_frame();
    }

    // Vérifie que le PC est dans une zone valide.
    let pc = m.cpu.state().pc;
    assert!(
        pc_in_valid_range(pc),
        "PC at 0x{:06X} not in BIOS [0xC00000-0xC1FFFF] or RAM [0x100000-0x10FFFF]",
        pc
    );

    // Vérifie que le framebuffer contient ≥ 2 couleurs distinctes.
    let fb = m.framebuffer();
    let colors = count_colors(fb);
    assert!(
        colors >= 2,
        "Framebuffer has only {} distinct color(s), expected >= 2",
        colors
    );

    // Vérifie que la machine a bien exécuté des trames.
    assert_eq!(m.frame_count(), 180);
}

/// Test jalon avec BIOS synthétique (ne nécessite pas de fichiers locaux).
#[test]
fn bios_boot_synthetic() {
    use ng_cart::{find_def, Bios, Cart};

    // BIOS synthétique : SP = 0x10F300, PC = 0xC00402.
    let mut main = vec![0u8; 0x20000];
    main[0..4].copy_from_slice(&0x0010_F300u32.to_be_bytes());
    main[4..8].copy_from_slice(&0x00C0_0402u32.to_be_bytes());
    // Programme : boucle NOP infinie (4E 71 = NOP, 60 FC = BRA.S .)
    main[0x402] = 0x4E;
    main[0x403] = 0x71;
    main[0x404] = 0x60;
    main[0x405] = 0xFC;

    let bios = Bios {
        main,
        lo: vec![0; 0x20000],
        sfix: vec![0; 0x20000],
        sm1: vec![0xAA; 0x1000],
    };

    let cart = Cart {
        def: find_def("mslug").expect("definition mslug"),
        p: vec![0x12; 0x1000],
        s: vec![0; 0x100],
        m: vec![0x34; 0x2000],
        m_crypt: Vec::new(),
        v: vec![0x56; 0x400],
        c: vec![0; 0x100],
    };

    let mut m = Machine::new(cart, bios, ng_cart::Model::Mvs);

    // Exécute quelques trames pour vérifier que la machine ne panique pas.
    for _ in 0..5 {
        m.run_frame();
    }

    // Le PC doit être dans le BIOS (boucle NOP à 0xC00402).
    let pc = m.cpu.state().pc;
    assert!(
        (0xC00000..=0xC1FFFF).contains(&pc),
        "PC at 0x{:06X} not in BIOS range",
        pc
    );

    // La machine doit avoir exécuté des trames.
    assert_eq!(m.frame_count(), 5);
}

/// Test du comparateur de couleurs : deux images identiques => 0 pixel différent.
#[test]
fn color_comparison_identical() {
    let mut fb1 = ng_common::Framebuffer::new();
    let mut fb2 = ng_common::Framebuffer::new();

    // Remplit les deux avec la même couleur.
    for i in 0..fb1.pixels.len() {
        fb1.pixels[i] = 0x00FF0000; // Rouge
        fb2.pixels[i] = 0x00FF0000; // Rouge
    }

    let colors1 = count_colors(&fb1);
    let colors2 = count_colors(&fb2);

    assert_eq!(colors1, 1, "Identical red pixels should have 1 color");
    assert_eq!(colors2, 1, "Identical red pixels should have 1 color");
}

/// Test du comparateur de couleurs : une image avec 1 pixel modifié => 2 couleurs.
#[test]
fn color_comparison_one_pixel_different() {
    let mut fb = ng_common::Framebuffer::new();

    // Remplit tout en rouge.
    for i in 0..fb.pixels.len() {
        fb.pixels[i] = 0x00FF0000; // Rouge
    }

    // Modifie un seul pixel en bleu.
    fb.pixels[100] = 0x000000FF; // Bleu

    let colors = count_colors(&fb);
    assert_eq!(colors, 2, "One different pixel should give 2 colors");
}

/// Test de la validation du PC.
#[test]
fn pc_range_validation() {
    // Dans le BIOS : valide.
    assert!(pc_in_valid_range(0xC00402));
    assert!(pc_in_valid_range(0xC1FFFF));

    // Dans la RAM : valide.
    assert!(pc_in_valid_range(0x100000));
    assert!(pc_in_valid_range(0x10FFFF));

    // Hors des zones valides : invalide.
    assert!(!pc_in_valid_range(0x000000));
    assert!(!pc_in_valid_range(0xC20000));
    assert!(!pc_in_valid_range(0x110000));
}

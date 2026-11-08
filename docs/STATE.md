# STATE — journal de passation (garder < 150 lignes)

## Phase courante
P3 Bus, BIOS, cartouches. T303 terminée (2026-11-08). Prochaine : T304.

### Fait — T303 ng-bus latch système, registres E/S, watchdog, commandes son
- crates/ng-bus/src/latch.rs : SystemLatch 74HC259 (Q0-Q7), écriture par décodage d'adresse (bit A1-A3 = sortie, bit A4 = valeur)
- crates/ng-bus/src/io.rs : struct Io implémentant Device — lectures $300000/$300080/$320000/$340000/$380000, écritures watchdog kick ($300001), commande son ($320000), ports $3800xx
- crates/ng-bus/src/watchdog.rs : Watchdog avec timeout 3_244_030 mclk (~135ms)
- 44 tests unitaires passent (latch, I/O reads/writes, watchdog)
- Gate OK (fmt, clippy -D warnings, tests workspace)

### Piège rencontré — T303
- Les écritures E/S Neo Geo utilisent la parité d'adresse pour sélectionner le byte : adresses paires → byte bas du mot, impaires → byte haut. Les flags upper/lower de l'instruction CPU ne sont pas utilisés par le décodage matériel des registres I/O.

### Fait — T302 ng-bus carte mémoire 68000 (RAM, ROM, BIOS, zones)
- crates/ng-bus/src/device.rs : trait Device + CartPort + NullDevice
- crates/ng-bus/src/map68k.rs : Bus68k avec read16/write16 par zone (match a >> 20)
- Zones implémentées : vecteurs ($000000), cart P-ROM fixe ($000080+), work RAM + miroirs ($100000-$1FFFFF), fenêtre bankswitch $200000, I/O/LSPC ($3x0000), palette ($400000-$7FFFFF), memcard ($800000-$BFFFFF), BIOS ($C00000), backup RAM ($D00000)
- 16 tests unitaires passent (RAM, miroirs, byte writes, BIOS, vecteurs, bank select, open bus, périphériques NullDevice)
- Gate OK

### Piège rencontré — T302
- La spec de la carte dit `match a >> 16` mais les gardes utilisent `(a>>20)` ; en réalité il faut `match a >> 20` car chaque zone fait 1 Mo (0x100000). Avec >>16, l'adresse 0xC00000 donne 0xC0 et ne matche aucun bras.

### Fait — T301 ng-cart chargeur BIOS + configuration neogeo.toml
- crates/ng-cart/src/bios.rs : load_bios() avec WordSwap pour main BIOS, 4 ROMs (main/lo/sfix/sm1)
- crates/ng-cart/src/config.rs : parse_config/load_config depuis TOML, enum Model (Mvs/Aes)
- Dépendance ajoutée : toml 0.8 (--no-default-features --features parse)
- Tests : WordSwap vérifié ([0x12,0x34] -> [0x34,0x12]), fichier manquant, taille incorrecte (512KB), parsing TOML MVS/AES/clé manquante
- 6 tests unitaires passent (3 bios + 3 config)
- Clippy clean, fmt OK

### Fait — T300 ng-cart chargeur de jeux (zip ou dossier), régions, CRC
- crates/ng-cart/src/romset.rs : Source (Zip/Dir), load_cart, find_def, CartError
- Dépendance ajoutée : zip 9.4.2 (--no-default-features --features deflate)
- Implémentation exacte de l'algorithme spec : WordSwap, Byte, Word32Swap, ROM_CONTINUE, m1_reload
- CRC vérification optionnelle (check_crc paramètre)
- Recherche insensible à la casse pour les entrées zip
- 9 tests unitaires passent (WordSwap, Byte interleaved, Word32Swap, ROM_CONTINUE, M reload, BadCrc, Dir vs Zip, find_def)
- Clippy clean, fmt OK

### Fait — T006 ng-testkit comparateur d'images golden
- crates/ng-testkit/src/golden.rs : load_png_rgb, diff_frame, write_diff_ppm
- Dépendance ajoutée : png (décodage PNG)
- 5 tests unitaires passent (roundtrip PNG, diff identique, pixel modifié, taille erronée, écriture PPM)
- Gate OK

### Fait — T005 Dépendances CPU : puce-m68k et puce-z80 (git épinglé) et test de fumée
- cargo fetch + build -p ng-core OK (dépendances git épinglées dans Cargo.toml)
- Test de fumée crates/ng-core/tests/cpu_crates.rs écrit avec 3 tests :
  - test_m68k_reset_move_stop : reset à 40 ticks, MOVE.W à 48 ticks, STOP puis step()=4 ticks
  - test_m68k_wait_states : MOVE.W sans attente = 16 ticks, avec wait_clocks(2) = 18 ticks
  - test_z80_step_api : LD A,1 ; ADD A,A ; HALT → A=2 et 15 T-states
- Gate OK (cargo fmt --check, clippy clean, tests passent sur tout le workspace)

### Fait — T004 Configuration locale (neogeo.example.toml)
- neogeo.example.toml créé avec chemins BIOS/ROMs/saves et config système (mvs, sp-s2.sp1, euro)
- .gitignore contient déjà neogeo.toml (vérifié par git check-ignore)
- TOML valide (testé avec tomllib Python)
- Gate OK : 26 tests passent

### Fait — T001 ng-common horloge, framebuffer, entrées, CRC
- clock.rs : constantes de timing Neo Geo (MCLK_PER_*, PIXELS_PER_LINE, etc.)
- video.rs : Framebuffer 320x224 RGBA8 avec crc32() et write_ppm()
- input.rs : PadState, SystemButtons, trait InputProvider (bits actifs-BAS)
- crc.rs : CRC-32/ISO-HDLC sans dépendance externe (table OnceLock)
- lib.rs : ré-exports publics de toutes les interfaces gelées
- 19 tests unitaires passent (timing, framebuffer, CRC, PPM, entrées)
- Clippy clean pour ng-common ; fmt OK

### Piège rencontré — T001
- write_file bloque si le fichier a été lu avec pagination ; utiliser patch ou re-lire sans offset/limit.
- La table CRC doit être nommée `table()` (snake_case) sinon clippy warning.

### Fait — T000 Workspace Cargo vérifié, outillage de gate, métriques
- tools/check_metrics.ps1 écrit et testé (détection de régression, création de baseline)
- Tests triviaux ajoutés dans les 7 crates lib.rs (ng-gui exclu, main.rs)
- .gitignore déjà complet (neogeo.toml inclus)
- Gate OK : cargo check + clippy + tests passent sur tout le workspace

### Piège rencontré — T000
- PowerShell interprète `$name` comme une référence de lecteur ; utiliser `${name}` dans les chaînes interpolées.

## Prochaine carte
T000 (workspace) -> T001 -> T004 -> T005 (dépendances CPU) -> T006, puis phases suivantes (voir tasks/INDEX.md). Profil Hermes : `rust-dev`.

## Décisions
- 2026-11-08 : RESET v5. CPU = crates de l'utilisateur `puce-m68k` et `puce-z80` (git épinglé, voir Cargo.toml et ref/cpu-crates/README.md). Anciennes cartes/crates archivés dans `_to_delete/reset_20261108/`. 65 cartes, toutes pour le profil `rust-dev`. Rien n'a encore été compilé sur le PC de l'utilisateur.
- 2026-11-02 : projet initialisé avec méthode v2 (cartes, oracles, gate, budget de contexte).

## Pièges connus
(vide)

## Blocages
(vide)

## Cartes régénérées (v4, audit complet du 2026-11-02)
65 cartes (v5, reset du 2026-11-08) dans tasks/ (INDEX.md = ordre des dépendances ; board.json ; scripts/kanban_import.ps1). Les générations précédentes sont dans _to_delete/old_tasks et _to_delete/old_tasks_v2.
- Audit automatique : 0 alerte (dépendances acycliques et existantes, plages de lignes valides, lectures source <= ~1100 lignes, aucune formule vague).
- Les tables générées (crates/ng-lspc/src/{dac,zoom}_table.rs, crates/ng-cart/src/{types,defs,cmc_tables}.rs) COMPILENT (cargo check --workspace vérifié côté cloud).
- Oracles GÉNÉRÉS (versionnés) : tests-data/ym (ymfm réel, 3 voies, T505), tests-data/prot (CRC des fonctions MAME réelles, T700/T701/T703a/T704/T705a), tests-data/scenes (rendu LSPC de référence Python, T406). Outils et conventions : tools/README-golden.md ; régénération : `powershell tools/oracles.ps1` (g++ + python). Pas de #[ignore] : leur absence fait échouer les tests. T712 supprimée. Limites : le brouillage M1 (cmc50_m1_decrypt) n'est vérifié que par le CRC MAME ; les scènes sprites sont synthétiques.
- Référence vidéo : MAME 0.287 sur X:\Mame_0287 ; `powershell tools/mame_golden.ps1 -Game mslug -Frames 120,600,1500 -Out tests-data/golden/mslug`.
- Nom de dépôt/exécution : le gate (scripts/gate.ps1) tourne sur le PC de l'utilisateur ; Claude ne compile que pour vérifier les tables générées.

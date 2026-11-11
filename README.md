# Puce16 Neo Geo

Émulateur Neo Geo (MVS/AES) cycle-exact en Rust, interface egui. Successeur de Puce16 (SNES).
Développé en programmation agentique avec un modèle local (contexte 135k) : voir `docs/METHOD.md`.

- Démarrer : `AGENTS.md`, puis `tasks/INDEX.md`.
- Spécs : `docs/spec/`, `ref/cpu-crates/` (docs des crates CPU puce-m68k et puce-z80).
- Aucun BIOS ni ROM commerciale n'est fourni ni commité.

## Jeux
**169 jeux testés** (voir `docs/CATALOGUE.md`, généré par `tools/gen_catalogue_md.py`) avec le BIOS Europe MVS v2 et les captures de référence MAME 0.287 :
- 600 trames : 53 `ok`, 9 `diff` (> 2 % de pixels différents de MAME), 107 `noir` (en majorité le BIOS encore à l'écran), 0 échec ;
- relance à 1500 trames des « noir » et d'un échantillon : 155 jeux `ok` au total, 10 `diff`, 4 « noir » (homebrews qui tournent, peu de couleurs) ;
- à 1500 trames, mslug3, garou, fatfury3 et rbff2 divergent de MAME et kof94 est sombre : voir `docs/QUESTIONS.md`.
11 jeux (homebrews / conversions) ne sont pas définis par MAME 0.287 et n'ont donc pas de comparaison.

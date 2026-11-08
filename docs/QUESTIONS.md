# Questions ouvertes (à trancher par l'utilisateur)

1. **Cible** : MVS, AES, ou les deux (BIOS différents) ? Hypothèse : MVS d'abord, AES ensuite.
2. **BIOS** : tu fournis neogeo.zip / uni-bios en local ? (jamais commité).
3. **Jeux visés pour valider** : lesquels (homebrew uniquement, ou jeux possédés) ?
4. **Protections/cryptage** (CMC, PCM2, etc.) : dans le périmètre ? Hypothèse : plus tard (T803).
5. **Plateformes** : Windows seul ou aussi Linux/macOS ? Hypothèse : Windows d'abord.
6. **Précision visée sur la vidéo** : cycle-exact au pixel (T408) ou ligne exacte au début ? Hypothèse : ligne d'abord, pixel ensuite.
7. **Comparaison avec MAME** : traces de référence disponibles sur ta machine ? (utile pour T800).
8. **Outil d'orchestration** : cartes importées dans Hermes Kanban depuis `tasks/board.json` — format d'import attendu ?
9. **Parallélisme** : un seul modèle local à la fois, ou plusieurs sessions simultanées (P1 || P2) ?

- Wait states autres que ROM/VRAM (spec 00 « A COMPLETER »), bus ouvert, détails PAL, vérif DAC, ordre octets C ROM : à trancher avant T601/T405.
- Pas d'oracle matériel vidéo : MAME = référence pixel.


## Décisions et points ouverts (audit 2026-11-02)
- [RÉSOLU] Compilation : `cargo test` s'exécute chez l'utilisateur (scripts/gate.ps1) ; les oracles (ymfm, protections MAME, rendu Python) sont déjà générés par Claude dans tests-data/ (voir tools/README-golden.md).
- [RÉSOLU] Référence vidéo : MAME 0.287 (`X:\Mame_0287`) ; captures PNG produites par tools/mame_golden.ps1 + tools/mame_snap.lua ; ROMs de test matérielles du wiki : T902.
- [OUVERT] Q-WAIT : wait states des zones RAM/BIOS/VRAM/palette/E-S non documentés (MAME : 0). À mesurer avec une ROM de test matérielle (T901).
- [OUVERT] Q-BIOS512 : BIOS 512 Kio (sp-45/sp-j3) non pris en charge (T301).
- [OUVERT] Q-PAR : ratio d'aspect (pixel 1:1 vs 4:3) à l'affichage (T800a).
- [OUVERT] Q-AUDIOCLK : l'audio n'est pas l'horloge maître de l'émulation (T801a).
- [RÉSOLU v5] Q-Z80OVER : Z80 cadencé au T-state par l'API broches de puce-z80 (T305/T600b).
- [OUVERT] Q-M68KSTATE : `puce_m68k::state::CpuState` ne contient pas l'état STOP/arrêt ni la phase de reset : un savestate pris pendant STOP n'est pas fidèle (T604). À traiter côté crate puce-m68k si nécessaire.

- [OUVERT] ROMs de test matériel (T902) : à déposer par l'utilisateur dans tests-data/hwtests/<nom>/ et lister dans docs/spec/data/test-roms.md.

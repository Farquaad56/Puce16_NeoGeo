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
- [OUVERT] Q-WAIT : wait states des zones RAM/BIOS/VRAM/palette/E-S non documentés (MAME : 0). À mesurer avec une ROM de test matérielle (T901). Wait states réels non mesurables sans le hardware : table à remplir si une ROM de test hardware les révèle.
- [OUVERT] Q-BIOS512 : BIOS 512 Kio (sp-45/sp-j3) non pris en charge (T301).
- [OUVERT] Q-PAR : ratio d'aspect (pixel 1:1 vs 4:3) à l'affichage (T800a).
- [OUVERT] Q-AUDIOCLK : l'audio n'est pas l'horloge maître de l'émulation (T801a).
- [RÉSOLU v5] Q-Z80OVER : Z80 cadencé au T-state par l'API broches de puce-z80 (T305/T600b).
- [OUVERT] Q-M68KSTATE : `puce_m68k::state::CpuState` ne contient pas l'état STOP/arrêt ni la phase de reset : un savestate pris pendant STOP n'est pas fidèle (T604). À traiter côté crate puce-m68k si nécessaire.

- [OUVERT] ROMs de test matériel (T902) : à déposer par l'utilisateur dans tests-data/hwtests/<nom>/ et lister dans docs/spec/data/test-roms.md.

- [PARTIEL T901] Bus ouvert : option `open_bus_follows_prefetch` disponible (défaut faux = $FFFF comme MAME). À activer si une ROM de test matérielle (T902) montre que le matériel renvoie le dernier mot préchargé.
- [OUVERT T901] Ordre des événements à égalité de tick et table de wait states : aucun écart MAME constaté, rien changé.

- (T1006) kof2001/kof96/lastblad/samsho4 : la chronologie de démarrage du jeu est en retard sur MAME (BIOS identique jusqu'à ~580 trames). Cause à étudier : rythme du handshake son Z80, chien de garde, ou durée d'un écran d'attente.

## T1014 - jockeygp : déroulement différent de MAME à 600 trames
Notre émulation affiche l'écran de mise (programme en cours) alors que MAME montre l'écran-titre (attract). La RAM de cartouche étant mappée, le jeu tourne ; reste à comparer l'état initial de la NVRAM (MAME) et les bits d'entrée SYSTEM (0x0100 / 0x0400 « freeze »). Non bloquant.

## T1018 - bilan du catalogue : jeux non conformes (banc 600 / 1500 trames, BIOS sp-s2)
- kof2000, kof2001, kof96, lastblad, samsho4 (600 trames, diff ~94 %) : chronologie de démarrage du jeu en retard sur MAME (BIOS identique) ; hypothèse : rythme du handshake son Z80 (voir T1006).
- kof99 (diff 33 % à 600) : niveau de fondu différent ; hypothèse : même cause que ci-dessus (décalage de quelques trames).
- fatfury2, ssideki (diff ~95 % à 600) : même écran que MAME, niveau de fondu différent ; hypothèse : idem.
- jockeygp (diff 91 % à 600) : écran de mise au lieu du titre ; hypothèse : état initial de la NVRAM ou bits d'entrée SYSTEM.
- mslug3 (diff 83 % à 1500), garou (32 %), fatfury3 (89 %), rbff2 (2,2 %) : déroulement différent de MAME à 1500 trames alors que 600 trames est conforme ; hypothèse : dérive de timing (fond d'écran/animations) à long terme.
- kof94 : écran sombre à 1500 trames (8 couleurs) alors qu'il est conforme à 600 ; pas de golden à 1500 (MAME bloque sur ce jeu) ; hypothèse : fondu entre deux écrans, à vérifier par un dump à d'autres trames.
- cnbe, ltorb, neothund, ngem2k : classés « noir » (< 16 couleurs) mais tournent (démo / jeux à palette réduite) : faux négatifs de la classification, pas de golden possible (hors MAME).
- irrmaze : pas de golden comparable (autre résolution due au layout MAME) ; actif à 1500 trames.

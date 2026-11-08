# Index des tâches (CHAÎNE LINÉAIRE : chaque carte dépend de la précédente ; profil rust-dev)

| Ordre | Id | Phase | Titre |
|---|---|---|---|
| 1 | T000 | P0 Fondations | Workspace Cargo vérifié, outillage de gate, métriques |
| 2 | T001 | P0 Fondations | ng-common : horloge, framebuffer, entrées, CRC |
| 3 | T004 | P0 Fondations | Configuration locale (neogeo.example.toml) |
| 4 | T005 | P0 Fondations | Dépendances CPU : puce-m68k et puce-z80 (git épinglé) et test de fumée |
| 5 | T006 | P0 Fondations | ng-testkit : comparateur d'images golden (PNG de MAME contre trame) |
| 6 | T300 | P3 Bus, BIOS, cartouches | ng-cart : chargeur de jeux (zip ou dossier), régions, CRC |
| 7 | T301 | P3 Bus, BIOS, cartouches | ng-cart : chargeur BIOS + configuration neogeo.toml |
| 8 | T302 | P3 Bus, BIOS, cartouches | ng-bus : carte mémoire 68000 (RAM, ROM, BIOS, zones) |
| 9 | T303 | P3 Bus, BIOS, cartouches | ng-bus : latch système, registres E/S, watchdog, commandes son |
| 10 | T304 | P3 Bus, BIOS, cartouches | ng-bus : backup RAM, mémoire carte (stub), RTC uPD4990A |
| 11 | T305 | P3 Bus, BIOS, cartouches | ng-bus : carte mémoire Z80, banking, ports, latches (SoundBoard + serveur de broches) |
| 12 | T306 | P3 Bus, BIOS, cartouches | ng-bus : banque P ROM ($2FFFF0) |
| 13 | T307 | P3 Bus, BIOS, cartouches | Différences AES (console) |
| 14 | T400 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : structure Lspc, VRAM, registres $3C0000, auto-animation |
| 15 | T401 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : temps vidéo, IRQ1/IRQ2/IRQ3, timer d'affichage, animation auto, début de ligne |
| 16 | T402 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : palette 2 banques, conversion couleur (table DAC) |
| 17 | T403 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : couche fix (une ligne), banking Garou/KOF2000 |
| 18 | T404 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : analyse des sprites d'une ligne (parse_sprites) |
| 19 | T405a | P4 Vidéo LSPC (pixel perfect) | ng-lspc : dessin des sprites d'une ligne (zoom, flips, animation, décodage C ROM) |
| 20 | T406 | P4 Vidéo LSPC (pixel perfect) | ng-lspc : composition d'une ligne et boucle de rendu (début de ligne), test sur scènes de référence |
| 21 | T500 | P5 Audio YM2610 | ng-ym2610 : types communs ymfm (interface, sorties, utilitaires) |
| 22 | T501a | P5 Audio YM2610 | ng-ym2610 : ADPCM-A (registres, canal, moteur) |
| 23 | T501b | P5 Audio YM2610 | ng-ym2610 : ADPCM-B (registres, canal, moteur) |
| 24 | T502 | P5 Audio YM2610 | ng-ym2610 : SSG (générateur de sons carrés/bruit) |
| 25 | T503a | P5 Audio YM2610 | ng-ym2610 : tables et fonctions FM (sin, atténuation, détune, LFO) |
| 26 | T503b | P5 Audio YM2610 | ng-ym2610 : registres FM OPN (fm_registers_base + opn_registers_base) |
| 27 | T503c | P5 Audio YM2610 | ng-ym2610 : fm_operator (enveloppe, phase, SSG-EG) |
| 28 | T503d | P5 Audio YM2610 | ng-ym2610 : fm_channel (algorithmes 2 et 4 opérateurs) |
| 29 | T503e | P5 Audio YM2610 | ng-ym2610 : fm_engine (timers, LFO, enveloppe globale) |
| 30 | T504a | P5 Audio YM2610 | ng-ym2610 : YM2610 (registres, statut, lecture/écriture) |
| 31 | T504b | P5 Audio YM2610 | ng-ym2610 : YM2610 génération audio (prescale, rééchantillonnage SSG) |
| 32 | T505 | P5 Audio YM2610 | Test de comparaison YM2610 aux vecteurs ymfm (oracle fourni) |
| 33 | T506 | P5 Audio YM2610 | Sortie audio : mélange YM (gains MAME) + rééchantillonnage vers l'hôte |
| 34 | T600a | P6 Intégration cycle-exact | ng-core : structure Machine, construction et reset |
| 35 | T600b | P6 Intégration cycle-exact | ng-core : ordonnanceur à l'horloge (68000 maître, Z80 au T-state, catch-up des autres puces) |
| 36 | T600c | P6 Intégration cycle-exact | ng-core : séquence de démarrage Z80, reset conjoint, boucle de trame publique |
| 37 | T601 | P6 Intégration cycle-exact | ng-core : wait states du bus 68000 |
| 38 | T602 | P6 Intégration cycle-exact | Démarrage du BIOS jusqu'à l'écran (test d'intégration) |
| 39 | T603 | P6 Intégration cycle-exact | Metal Slug 1 : démarrage et attract mode |
| 40 | T604 | P6 Intégration cycle-exact | Savestates (sérialisation complète) |
| 41 | T700 | P7 Protections Metal Slug | ng-cart : bitswap + déchiffrement ROM C (CMC42 / CMC50) |
| 42 | T701 | P7 Protections Metal Slug | ng-cart : sfix_decrypt, cmc50_m1_decrypt |
| 43 | T702 | P7 Protections Metal Slug | Metal Slug X : protection runtime ($2FFFE0-$2FFFEF) |
| 44 | T703a | P7 Protections Metal Slug | ng-cart : SMA Metal Slug 3 — déchiffrement P ROM |
| 45 | T703b | P7 Protections Metal Slug | ng-bus : SMA Metal Slug 3 — registres runtime (banque $2FFFE4, lecture $2FE446) |
| 46 | T704 | P7 Protections Metal Slug | ng-cart : PCM2 (ROM V) — decrypt et swap |
| 47 | T705a | P7 Protections Metal Slug | ng-cart : PVC Metal Slug 5 — déchiffrement P ROM |
| 48 | T705b | P7 Protections Metal Slug | ng-bus : PVC Metal Slug 5 — RAM de protection $2FE000-$2FFFFF |
| 49 | T706 | P7 Protections Metal Slug | Validation mslug2 (démarrage + attract) |
| 50 | T707 | P7 Protections Metal Slug | Validation mslugx (démarrage + attract) |
| 51 | T711 | P7 Protections Metal Slug | ng-cart/ng-core : chaîne de déchiffrement par jeu et installation des ports |
| 52 | T708 | P7 Protections Metal Slug | Validation mslug3 (démarrage + attract) |
| 53 | T709 | P7 Protections Metal Slug | Validation mslug4 (démarrage + attract) |
| 54 | T710 | P7 Protections Metal Slug | Validation mslug5 (démarrage + attract) |
| 55 | T800a | P8 GUI egui | ng-gui : application eframe, thread d'émulation, affichage de la trame |
| 56 | T800b | P8 GUI egui | ng-gui : menu Fichier (ouvrir un jeu), liste des jeux, pause/reset, capture d'écran |
| 57 | T801a | P8 GUI egui | ng-gui : sortie audio (cpal) avec tampon circulaire |
| 58 | T801b | P8 GUI egui | ng-gui : entrées clavier et manettes (gilrs), table de touches configurable |
| 59 | T802a | P8 GUI egui | ng-gui : débogueur — registres CPU et visionneuse mémoire |
| 60 | T802b | P8 GUI egui | ng-gui : débogueur — VRAM, palette, sprites |
| 61 | T900 | P9 Précision, outils, AES | Banc de performance (criterion) et mesure de référence |
| 62 | T900b | P9 Précision, outils, AES | Optimisation : catch-up paresseux sans changer le résultat |
| 63 | T901 | P9 Précision, outils, AES | Précision restante : bus ouvert, PAL, wait states mesurés (liste QUESTIONS) |
| 64 | T902 | P9 Précision, outils, AES | ROMs de test matérielles (wiki neogeodev) : intégration et captures MAME |
| 65 | T903 | P9 Précision, outils, AES | Carte mémoire (memory card) et AES : validation |

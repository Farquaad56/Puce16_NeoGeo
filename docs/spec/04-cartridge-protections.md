# 04 — Cartouches, chargement des ROM et protections (priorité Metal Slug)
Sources : MAME src/devices/bus/neogeo/* et neogeo.cpp [BSD-3] (copies dans ref/mame-neogeo/), wiki NEO-CMC, Bankswitching, Cartridge_ROM_arrangements, Memory_speed.

## Régions d'une cartouche (noms MAME) et rôle
| Région | Fichier(s) | Rôle |
|---|---|---|
| maincpu (P ROM) | *-p1.p1, *-p2.sp2 | code 68000, 16 bits big-endian |
| fixed (S ROM) | *-s1.s1 | tuiles fix |
| audiocpu (M1) | *-m1.m1 | code Z80 (+ données) |
| ymsnd:adpcma (V ROM) | *-v1.v1, v2... | échantillons ADPCM-A (et B) |
| sprites (C ROM) | *-c1.c1, c2.c2... | tuiles sprites, paires (c1,c2), (c3,c4)... chargées en `ROM_LOAD16_BYTE` : c1 aux octets pairs, c2 aux octets impairs |
Les définitions exactes des jeux Metal Slug (tailles, offsets, CRC, ordre de chargement) : `docs/spec/data/romsets_metalslug.txt` (extrait de MAME ; contient aussi les noms de fichiers du BIOS MVS). L'émulateur charge soit un dossier de fichiers, soit un zip MAME ; la table de correspondance est dans un fichier TOML par jeu `games/<nom>.toml` (voir carte T301).
P ROM `ROM_LOAD16_WORD_SWAP` : chaque paire d'octets du fichier est échangée (fichier little-endian de la ROM -> mots big-endian). Cas mslug : `201-p1.p1` (2 Mio) : 1er Mio chargé à $100000 de la région, 2e Mio (ROM_CONTINUE) à $000000.

## Banc P ROM ($200000-$2FFFFF)
Banc par défaut = octets $100000-$1FFFFF de la région maincpu (base $100000 si la ROM > 1 Mio, sinon 0). Écriture MOT dans $2FFFF0-$2FFFFF (n'importe quelle adresse de cette plage) : `write_banksel` -> banc n : adresse ROM = (n+1) * $100000. Bits utilisés : dépend de la taille (voir MAME write_banksel, neogeo.cpp ~l.1225).
Vecteurs $000000-$00007F : lus depuis le BIOS (SWPBIOS) ou la cartouche (SWPROM) selon le latch Q1 (voir 01). $000080-$0FFFFF : toujours la cartouche.

## Metal Slug — cartouches (type MAME -> protection)
| Jeu | Type MAME | Puces | Déchiffrement requis |
|---|---|---|---|
| Metal Slug (NGH 201) | rom (aucun) | aucune | **aucun** — ROM en clair |
| Metal Slug 2 (241) | rom | aucune | **aucun** |
| Metal Slug X (250) | rom_mslugx | protection P ROM par puce ALTERA (registres $2FFFE0-$2FFFEF) | pas de chiffrement ; émulation de registres de protection (voir prot_misc.cpp mslugx_*) |
| Metal Slug 3 (256) | sma_mslug3 | SMA + CMC42 | `mslug3_decrypt_68k` + `cmc42_gfx_decrypt(clé 0xAD)` ; banc P via $2FFFE4 (`mslug3_bank_base`) ; lecture protection $2FE446 |
| Metal Slug 4 (263) | pcm2_mslug4 | PCM2 + CMC50 | `cmc50_m1_decrypt` + `cmc50_gfx_decrypt(clé 0x31)` + `sfix_decrypt` + `pcm2.decrypt(V ROM, 8)` |
| Metal Slug 5 (268) | pvc_mslug5 | PVC + PCM2 + CMC50 | `pvc.mslug5_decrypt_68k` + `pcm2.swap(V ROM, 2)` + `cmc50_m1_decrypt` + `cmc50_gfx_decrypt(clé 0x19)` + `sfix_decrypt` ; RAM de protection PVC $2FE000-$2FFFFF (prot_pvc.cpp) |
Ordre de jeu : MS1 -> MS2 -> MSX -> MS3 -> MS4 -> MS5 (du plus simple au plus complexe). Le chiffrement n'a lieu qu'au chargement (déchiffrement une fois en mémoire) : aucun impact sur le cycle-exact. SEULS les registres de protection runtime (MSX, MS3 bank/prot, MS5 PVC) s'exécutent pendant l'émulation.
Source de TOUS les algorithmes : ref/mame-neogeo/prot_cmc.cpp (CMC42/50, tables), prot_pcm2.cpp, prot_sma.cpp, prot_pvc.cpp, prot_misc.cpp ; câblage par jeu : sma.cpp, cmc.cpp, pcm2.cpp, pvc.cpp. Le wiki NEO-CMC ne contient PAS l'algorithme (page "Todo").
Les fonctions à porter en Rust 1:1 (traduction C++ -> Rust, mêmes noms) : cmc42_gfx_decrypt, cmc50_gfx_decrypt, cmc50_m1_decrypt, sfix_decrypt, pcm2 decrypt/swap, mslug3_decrypt_68k, mslug5_decrypt_68k, mslug3_bank_base.

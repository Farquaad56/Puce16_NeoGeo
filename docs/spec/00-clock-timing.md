# 00 — Horloges et timing vidéo (source : wiki neogeodev : Clock, Display_timing, Rendering_logic, Wait_cycle, Memory_speed, VRAM)

## Horloge maître (mclk)
- MVS : 24 MHz. AES : 24.167829 MHz (le reste est identique, tout est dérivé de mclk).
- 68000 : 12 MHz = mclk/2. Z80 : 4 MHz = mclk/6. YM2610 : 8 MHz = mclk/3. Pixel LSPC : 6 MHz = mclk/4. NEO-ZMC2 : 12 MHz.
- Phase : le signal 8M n'est pas 50/50 (33% haut / 66% bas, d'après le wiki, à-faire sur la page).
- RTC (MVS) : quartz 32.768 kHz, indépendant.
- Convention du projet : 1 tick = 1 mclk. CPU 68k = 2 ticks/cycle, Z80 = 6, YM = 3, pixel = 4.

## Ligne vidéo (1 pixel = 4 mclk)
| Segment | mclk | pixels |
|---|---|---|
| H-sync | 112 | 28 |
| Back porch | 112 | 28 |
| Affichage actif | 1280 | 320 |
| Front porch | 32 | 8 |
| **Total ligne** | **1536** | **384** |
Blanking horizontal = 256 mclk = 64 px (porches + sync). La compteur de pixels sur 384 valeurs.
Ordre dans la ligne (wiki : BNKB change 14 px après H-sync, au milieu du back porch) : sync -> back porch -> actif -> front porch.

## Trame NTSC (264 lignes) — valeurs du compteur de lignes (9 bits hauts de REG_LSPCMODE)
| Zone | lignes | compteur |
|---|---|---|
| V-sync | 8 | $0F8-$0FF |
| Bordure haute | 16 | $100-$10F (visible en PAL, masquée en NTSC) |
| Actif | 224 | $110-$1EF |
| Bordure basse | 16 | $1F0-$1FF |
Le compteur compte de $0F8 à $1FF (264 valeurs) puis revient à $0F8.
Cadence NTSC : 264 x 1536 = 405 504 mclk/trame -> 59,185 Hz (MVS 24 MHz). PAL : 312 lignes (50 Hz, sélection LSPC2 seulement). Le wiki est incohérent sur PAL : à confirmer (voir QUESTIONS.md).
Page Framerate/Frame size non lues : A COMPLETER.

## Pipeline de rendu (Rendering_logic — page marquée "draft")
- Pixels fix : 6 MHz, 1 pixel / 4 mclk. Pixels sprites : 2 pixels / 2 mclk (12 MHz).
- Ligne tuile fix : 32 mclk (8 px). Ligne tuile sprite : 16 mclk (16 px), constant même avec zoom.
- Évaluation (parsing) de la ligne N pendant la ligne N-2 ; rendu de la ligne N pendant la ligne N-1 ; sortie pendant N.
- Chaque ligne : 381 lectures SCB3 (toujours, match ou pas). Le sprite #0 ne peut pas être utilisé (il remplit les entrées inutilisées de la liste active).
- Liste active : max 96 entrées par ligne, deux listes en VRAM rapide ($8600 paire, $8680 impaire), alternées chaque ligne. Écrite seulement pour les sprites qui matchent.
- Cycle de parsing : 16 mclk ; une ligne = 96 cycles = 480 slots de 5 par cycle (d'après le wiki).
- Deux line buffers de 320 pixels, chacun découpé en 4 buffers entrelacés de 160 px (pair/impair) ; ils basculent à chaque ligne (bit TMS0 = bit 0 du compteur de ligne). Écriture de pixels dans n'importe quel ordre.
- Fix : dessiné par-dessus la sortie du buffer sprite en temps réel ; 4 lectures S ROM par ligne de tuile, 2 px par lecture à 4 bpp ; données prélevées 8 px (32 mclk) à l'avance.
- Slots VRAM lente (4 slots de 4 mclk) : lecture map sprite paire, lecture map sprite impaire, accès CPU, lecture map fix.
- VRAM rapide : 10 slots, 16 mclk total (parsing 2 + 4x1.5 ; rendu 2 + 4x1.5 ; lecture liste active 2 mclk ; SCB2/3/4 et CPU à 1.5 mclk).
- Accès CPU : au moins 12 cycles 68k (24 mclk) entre deux écritures => max 64 écritures par ligne.
- NON documenté sur la page : priorités sprite/fix, valeurs exactes hblank/vblank internes -> à lire sur Graphics_pipeline, Line_buffers, Sprites, Sticky_bit (cartes T012x).

## VRAM (page VRAM)
Adresses = mots de 16 bits. Hors espace d'adressage du 68k, accès par REG_VRAMADDR/REG_VRAMRW/REG_VRAMMOD.
| Début | Fin | Zone | Contenu |
|---|---|---|---|
| $0000 | $6FFF | basse (lente) | SCB1 |
| $7000 | $74FF | basse | Fix map (1280 mots = 40x32) |
| $7500 | $7FFF | basse | Extension (fix bankswitching) |
| $8000 | $81FF | haute (rapide) | SCB2 (512) |
| $8200 | $83FF | haute | SCB3 (512) |
| $8400 | $85FF | haute | SCB4 (512) |
| $8600 | $867F | haute | Liste de sprites lignes paires (128) |
| $8680 | $86FF | haute | Liste de sprites lignes impaires (128) |
| $8700 | $87FF | haute | Inutilisé (256) |
- Lente : 64 KiB, accès max 120 ns (3 mclk). Rapide : 4 KiB, 35 ns (1 mclk).
- Passer d'une zone à l'autre ($7FFF -> $8000) impose de réécrire REG_VRAMADDR ; REG_VRAMMOD ne traverse pas.
- Délais minimaux CPU : écriture adresse -> lecture valide 16 cycles CPU (>32 mclk) ; écriture donnée -> nouvelle adresse 16 cycles ; donnée -> donnée 12 cycles (>24 mclk).

## Wait states du bus 68k (page Wait_cycle)
| Zone | Contenu | Wait |
|---|---|---|
| $000000-$0FFFFF | P ROM | 0 ou 1 cycle (pin ROMWAIT : 1=plein débit, 0=1 cycle) |
| $200000-$2FFFFF | PORT (P ROM banquée ou puces spéciales) | 0, 1, 2 cycles selon PWAIT1/PWAIT0 (11=0, 10=1, 01=2, 00=attente tant que PDTACK haut) |
| $800000-$BFFFFF | Carte mémoire | 2 cycles fixes |
Pins actives à bas niveau ; beaucoup de cartouches mettent les deux à 5 V (plein débit). Certains jeux ont besoin des waits pour espacer les accès VRAM.
Les wait states des autres zones (RAM, BIOS, VRAM, palette, I/O) ne figurent PAS sur la page : A COMPLETER (voir Memory_speed : P ROM = 6 mclk, soit 3 cycles 68k ; carte mémoire = 10 mclk).

## Temps d'accès mémoire (Memory_speed, valeurs partielles)
Z80 RAM <500 ns (12 mclk ?) ; VRAM basse 120 ns (3 mclk) ; VRAM haute 35 ns (1 mclk) ; Palette RAM 120 ns (3 mclk) ; P ROM 250 ns (6 mclk = 3 cycles 68k) ; V ROM 200 ns (6 mclk, lecture YM2610) ; S ROM/C ROM 200 ns (5 mclk ?) ; carte mémoire 417 ns (10 mclk = 3 cycles 68k + 2 waits) ; autres cases vides sur la page.

## Corrections vérifiées (2026-11-02, contre MAME 0.287)
- Watchdog : 3 244 030 mclk (neogeo.cpp, `WATCHDOG_TIMER`), remis à zéro par écriture octet à $300001.
- Wait states : seuls ROM P (zone $000000-$0FFFFF) et PORT ($200000-$2FFFFF) sont documentés ; MAME ne les modélise pas (0 cycle). Les autres zones restent un point OUVERT (docs/QUESTIONS.md Q-WAIT).
- Interruptions : IRQ1 (VBLANK) à vpos 240 + 58 mclk ; autoload du compteur à vpos 240 + 1146 mclk ; auto-animation à vpos 256 ; IRQ3 en attente au reset ; bits irq2_ctrl : 4 enable, 5 recharge relative sur écriture basse, 6 autoload vblank, 7 répétition.

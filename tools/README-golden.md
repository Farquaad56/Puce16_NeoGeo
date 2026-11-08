# Oracles (générés par Claude, exécutés avec g++ / python3)
Ces fichiers sont déjà générés et versionnés (tests-data/). Régénération : `powershell tools/oracles.ps1` (g++ requis ; sinon WSL).

## YM2610 — tests-data/ym/*.bin
Pilote `ymfm_golden.cpp` (ymfm BSD-3, fidélité MED = celle de MAME). Format : 1000 échantillons × 3 voies i16 LE = [FM gauche, FM droite, SSG mono] (6000 octets).
Fichiers : silence, fm, ssg, adpcma, adpcmb, timers (+ timers_status.bin : 1000 octets = `read(0)` après chaque échantillon), mix.
Conventions (identiques à l'adaptateur Rust, carte T600a) : 1 échantillon / 144 horloges YM ; l'échantillon n (n>=1) est produit à ym_time = n*144 ;
avant lui : (1) écritures planifiées « avant l'échantillon n » (ym_time = (n-1)*144), (2) timers dont l'échéance <= n*144, dans l'ordre chronologique
(`engine_timer_expired`), (3) `generate`. `set_timer(t,d)` : échéance = ym_time_au_moment_de_l_appel + d (d<0 : arrêt). `set_busy_end(c)` : busy jusqu'à ym_time+c.
ROM ADPCM (A et B) : `rom(a) = (h ^ (h>>15)) & 0xFF` avec `h = a.wrapping_mul(0x9E3779B1)` (u32). Les scénarios sont listés dans `ymfm_golden.cpp` (fonctions lo/hi : lo = registres 0x00-0xFF, hi = 0x100+).

## Protections — tests-data/prot/*.crc
Fonctions MAME réelles (prot_*.cpp) compilées avec un shim minimal (tools/prot_shim). Entrée : `pat(i) = ((i as u32).wrapping_mul(0x9E3779B1) >> 24) as u8`.
P ROM (MS3, MS5) : l'entrée est le tampon Rust `p` (big-endian 68000) ; le CRC (CRC-32/ISO-HDLC) porte sur `p` après déchiffrement, dans l'ordre Rust.
Cas : cmc42_010000_k00/kAD, cmc50_010000_k31/k19 (ROM C de 0x10000 octets), sfix_010000_1000 (fixed de 0x1000 octets, issu de la fin d'une ROM C de 0x10000),
m1_crypt_080000 / m1_audio_090000 (cmc50_m1_decrypt : romcrypt 0x80000, audio 0x90000 ; CRC des deux tampons), mslug3_p_900000, mslug5_p_800000,
pcm2_decrypt8_010000 (V ROM 0x10000, value 8), pcm2_swap2_1000000 (V ROM 0x1000000, value 2). Chaque .crc contient 8 chiffres hexadécimaux majuscules.
Vérification croisée : les spécifications des cartes T700, T701(sfix), T703a, T704, T705a ont été réimplémentées indépendamment en Python et reproduisent ces CRC.

## Rendu — tests-data/scenes/<scène>/
Scènes synthétiques déterministes (graine fixe, tools/gen_scenes.py) et rendu de référence `expected.raw` (tools/ref_render.py, port indépendant de neogeo_spr.cpp).
Fichiers d'une scène : vram.bin (0x8800 u16 LE), pens.bin (0x1000 u32 LE = tranche de palette de base ; backdrop = pens[0xFFF]), c.bin, s.bin, lo.bin, meta.txt (anim_counter, anim_disabled, fix_bank 0/1/2, from_cart 0/1), expected.raw (320×224 u32 LE 0x00RRGGBB).
Séquence de rendu = sprite_line_timer_callback : pour line in 0..264 { si line != 0 { render(line-1) } ; parse(line) } ; les lignes 16..239 sont écrites.

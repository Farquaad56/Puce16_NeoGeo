# 03 — Audio : Z80, YM2610, communication 68k<->Z80 (sources : MAME neogeo.cpp [BSD-3], wiki YM2610_registers, ymfm [BSD-3])
Références locales : ref/mame-neogeo/neogeo.cpp (l.1112-1116, 1300-1350, 1777-1805, 1930-1965), ref/ymfm/src/ymfm_opn.cpp (classe ym2610), ymfm_adpcm.cpp, ymfm_ssg.cpp, ymfm_fm.ipp.

## Z80 : horloge 4 MHz (mclk/6)
### Carte mémoire Z80
| Plage | Contenu |
|---|---|
| $0000-$7FFF | Fixe : 32 Kio bas de la M1 cartouche, OU de la SM1 du BIOS (sm1.sm1) selon le latch système (Q5 : REG_BRDFIX=BIOS / REG_CRTFIX=cart ; $3A000B / $3A001B) |
| $8000-$BFFF | Fenêtre 16 Kio banquée |
| $C000-$DFFF | Fenêtre 8 Kio banquée |
| $E000-$EFFF | Fenêtre 4 Kio banquée |
| $F000-$F7FF | Fenêtre 2 Kio banquée |
| $F800-$FFFF | RAM Z80 2 Kio |
### Banking (puce NEO-ZMC / équivalents) — se commande par LECTURE de port
`IN A,(C)` avec C bas = $08..$0B (miroir : bits 4-7 du port ignorés) et B (octet haut de l'adresse de port) = numéro de banque. La valeur lue est 0. Indice de fenêtre i = (port bas & 3) : 0 => fenêtre $F000 (2 Kio), 1 => $E000 (4 Kio), 2 => $C000 (8 Kio), 3 => $8000 (16 Kio).
Adresse dans la ROM M1 : `0x10000 + ((banque << (11 + i)) & masque)` avec `masque = (taille_M1 - 0x10000 - 1) & 0x3FFFF`. (la M1 > 64 Kio ; l'espace de banque commence à l'offset $10000 de la M1.)
Banques initiales au reset (identiques à MAME ; sur matériel réel = 0, mais nécessaire aux jeux sans banking) : fenêtre $F000 = $1E, $E000 = $0E, $C000 = $06, $8000 = $02.
### Ports d'E/S Z80 (seul l'octet bas compte, sauf banking)
| Port | Lecture | Écriture |
|---|---|---|
| $00 | lit le verrou de commande (68k -> Z80) | acquitte/efface le verrou (clear pending) |
| $04 | statut YM2610 (voir registres ; adresse port A) | adresse port A |
| $05 | donnée port A (lecture SSG si adresse $00-$0F) | donnée port A |
| $06 | statut ADPCM (bits fin de canal) | adresse port B |
| $07 | donnée port B (0) | donnée port B |
| $08 | (banking, voir plus haut) | $08 = ACTIVE la NMI ; $18 = DÉSACTIVE la NMI (bit 4 du port) |
| $0C | — | écrit le verrou de réponse (Z80 -> 68k) |
### Interruptions Z80
- INT (niveau, mode 1 => vecteur $38) = sortie IRQ du YM2610 (timers A/B).
- NMI = (verrou de commande non lu) ET (NMI activée par `out ($08)`). Le verrou est posé quand le 68k écrit l'octet de commande, effacé par un accès au port $00 EN LECTURE (`in a,($00)`, qui renvoie la commande) OU en écriture (`out ($00),a`) ; remis à zéro au reset avec l'activation de NMI (désactivée au reset). NMI active-sur-front.
### Communication 68k
- 68k écrit un OCTET à $320000 (miroir $01FFFE) : pose le verrou de commande (+ NMI Z80 si activée).
- 68k lit $320000 (mot) : octet haut = verrou de réponse (écrit par le Z80 au port $0C) ; octet bas = bits statut (voir 01 : coin/service/slot/RTC TP bit 6, RTC data out bit 7).
- MAME force un "perfect quantum" de 50 µs après chaque commande son (le jeu S1945p glitche sinon) : l'émulateur cycle-exact n'en a pas besoin (interleaving au cycle).
- Reset Z80 au démarrage ; commande de reset par le 68k via BIOS (REG_SOUND).

## YM2610 (8 MHz = mclk/3)
Implémentation = PORT DE ymfm (BSD-3) : struct `ym2610` de `ymfm_opn.cpp` (FM 4 voies, SSG, ADPCM-A 6 voies, ADPCM-B 1 voie). Les taux d'échantillonnage et prescalers sont ceux de ymfm (`ym2610::sample_rate()`, `ymfm_ssg.h`, `ymfm_adpcm.cpp`) : NE PAS les recalculer, les porter tels quels. Le port reproduit exactement les tables (sin/exp, pas ADPCM) de ymfm.
Ports Z80 : $04 adresse A, $05 donnée A, $06 adresse B, $07 donnée B (voir table plus haut).
Accès ROM (V ROM) : interface `ymfm::ymfm_interface::ymfm_external_read(access_class, address)` : ACCESS_ADPCM_A lit la région "adpcma", ACCESS_ADPCM_B lit la région "adpcmb" (si la romset n'a qu'une région `ymsnd` sans région B, ADPCM-B lit dans la même ROM que A). Lecture V ROM = 6 mclk (voir 00).
Mixage MAME (gains, à reproduire pour la parité) : FM+SSG sortie 0 -> 0.84 sur les deux haut-parleurs ; ADPCM-A sortie 1 -> 0.98 gauche, sortie 2 -> 0.98 droite. (neogeo_stereo(): add_route(0,"speaker",0.84,0 et 1) ; add_route(1,..,0.98,0) ; add_route(2,..,0.98,1)).
Statut/registres : voir wiki YM2610_registers (partiellement confus) — AUTORITÉ = ymfm.

## Corrections vérifiées (2026-11-02)
- Port $00 : lecture ET écriture effacent le verrou de commande en attente. Reset : NMI désactivée, verrou vide.
- YM2610 : fidélité MED (MAME `OPN_FIDELITY_MED`) : un échantillon stéréo toutes les 432 mclk ; seule fidélité à implémenter.

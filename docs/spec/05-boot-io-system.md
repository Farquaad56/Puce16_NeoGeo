# 05 — Boot, entrées/sorties, RTC, watchdog, SRAM, carte mémoire (source : MAME neogeo.cpp [BSD-3] + wiki)
## Reset / boot
- Au reset : IRQ3 en attente (posée à machine_start, PAS sur un reset ultérieur), NMI Z80 désactivée, verrou son effacé, vecteurs = BIOS (Q1=0), palette banc 0, shadow off.
- Le 68k lit SSP/PC aux vecteurs 0 et 4 ($000000/$000004) du BIOS (sp-s2.sp1 etc. ; mapping miroir $C00000).
- BIOS MVS : régions "mainbios" 128 Kio (ou 512 Kio pour sp-45/sp-j3), chargement WORD_SWAP. Fichiers annexes du jeu de BIOS neogeo.zip : `000-lo.lo` (ROM de zoom vertical L0, 128 Kio = 0x20000 octets), `sfix.sfix` (tuiles fix embarquées, 128 Kio), `sm1.sm1` (Z80 BIOS, 128 Kio). Liste complète dans docs/spec/data/romsets_metalslug.txt (macro NEOGEO_BIOS) et neogeo.cpp l.2253+.
- AES : mêmes ROM système mais registres d'entrée différents (aes_main_map l.1748+), pas de Backup RAM ($D00000 non mappé), carte mémoire à $800000-$BFFFFF, RTC absente, bit 15 de REG_STATUS_B = 0.

## Latch système (74HC259, MVS)
Écriture octet à $3A0000-$3A001F (miroir $01FFE0) : bits d'adresse A1-A3 = numéro de sortie Q0..Q7, bit d'adresse A4 = donnée. Q0 shadow, Q1 vecteurs cartouche (0=BIOS), Q2 carte mémoire lock1, Q3 carte mémoire unlock2, Q4 carte mémoire regsel, Q5 audio cartouche (1) / BIOS SM1 (0) [aussi fix : REG_BRDFIX=0 / CRTFIX=1], Q6 déverrouillage SRAM, Q7 banc de palette (0=banc 0, 1=banc 1).
Pour le fix : le sélecteur de source fix (`m_fixed_layer_source`) suit Q5 (carte ou BIOS).

## Backup RAM (MVS) $D00000-$D0FFFF
Écritures ignorées tant que Q6=0 (verrouillée). Persistante (fichier .sav 64 Kio). Miroir $D10000-$DFFFFF.

## Watchdog
Timeout 3 244 030 mclk (≈ 135 ms). Remis à zéro par une ÉCRITURE octet à $300001 (miroir $01FFFE). À expiration : reset machine complet.

## RTC uPD4990A (MVS)
Pilotée par REG_RTCCTRL ($380051, bits 0 DIN, 1 CLK, 2 STB) ; lue dans $320001 : bit 6 = TP (impulsion), bit 7 = DATA OUT. Référence d'implémentation : ref/mame-neogeo/upd1990a.cpp (upd4990a = upd1990a). TP doit être cohérent avec la fréquence réglée (64 Hz / 256 Hz / 2048 Hz ou 1 Hz) car le self-test BIOS mesure la vitesse ("CALENDAR ERROR").

## Entrées (MVS, actif bas)
- Lectures (MAME neogeo.cpp, ports en bits ACTIFS-BAS) : $300000 = (manette 1 << 8) | DIPSW (0xFF) ; $300080 = 0xFF00 | 0x40 (JAMMA) | (bit 7 = 0 si bouton Test pressé) ; $320000 = (réponse son << 8) | statut ; $340000 = (manette 2 << 8) | 0xFF ; $380000 (mot) : bits 8-11 = Start1, Select1, Start2, Select2 (actifs-bas) ; bits 12-14 = carte mémoire (0x7000 = absente) ; bit 15 = 1 sur MVS, 0 sur AES ; bits 0-7 = 0xFF.
- (REG_SYSTYPE $300080 : voir ligne précédente.)
- Octet bas de $320000 (statut) : bit 0 coin1, 1 coin2, 2 service, 3 coin3, 4 coin4 (tous actifs-bas), bit 5 = 0 (1 slot), bit 6 = TP RTC, bit 7 = DATA OUT RTC.
- DIP : $300001 en lecture.
- Sorties : $380001 REG_POUTPUT (sélection manette), $380011 banc carte, $380021 slot, $380031 latches LED, $380041 données LED, $380051 RTC, $380061.. compteurs/verrous pièces.

## Carte mémoire ($800000-$BFFFFF)
2 wait states (6 cycles/accès) ; activée par /UDS (octet haut) ; banc = REG_CRDBANK bits 0..2. Absente => lecture $FFFF. Fichier .mcr 2 Kio..8 Mio.

## Bus ouvert / zones non mappées
Lecture d'une zone non mappée renvoie le mot lu à l'adresse du PC courant (valeur de prefetch) (MAME unmapped_r) ; recursion => $FFFF. A raffiner en cycle-exact.

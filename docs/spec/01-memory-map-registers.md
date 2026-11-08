# 01 — Carte mémoire 68000 et registres (sources : wiki 68k_memory_map, Memory_mapped_registers, 68k_interrupts, Wait_cycle)

## Carte mémoire cartouche (MVS/AES). "clks" = cycles 68k par accès (4 = sans wait)
| Début | Fin | Contenu | Cycles/accès | Miroirs |
|---|---|---|---|---|
| $000000 | $0FFFFF | Table des vecteurs + 1er banc fixe de P ROM | 4 ou 5 (ROMWAIT) | |
| $100000 | $10F2FF | WORKRAM (RAM utilisateur, 62 208 octets) | 4 | |
| $10F300 | $10FFFF | RAM réservée BIOS (3 328 octets) | 4 | |
| $110000 | $1FFFFF | Miroir de la RAM ($100000-$10FFFF répété) | 4 | oui |
| $200000 | $2FFFFF | 2e banc P ROM / bankswitching / puces spéciales (PORT) | 4 à 7 (PWAIT) | |
| $300000 | $3FFFFF | Registres I/O, système, vidéo (voir ci-dessous) | 4 | masques |
| $400000 | $401FFF | Palette RAM (banc sélectionné par REG_PALBANK), 8 KiB | 4 | $402000-$7FFFFF miroir |
| $800000 | $BFFFFF | Carte mémoire (jusqu'à 8 MiB, 8 bits) | 6 (2 waits) | |
| $C00000 | $C1FFFF | System ROM (BIOS) 128 KiB | 4 | $C20000-$CFFFFF miroir |
| $D00000 | $D0FFFF | Backup RAM 64 KiB (MVS seulement) | 4 | $D10000-$DFFFFF miroir |
Remarque : le wiki indique WORKRAM de 62 208 octets ($100000-$10F2FF) puis RAM BIOS ; ne pas coder "64 KiB" brut. Accès hors zones : comportement open-bus non documenté (A COMPLETER, QUESTIONS.md).
La page ne donne pas la largeur d'accès par zone. Les registres vidéo ($3C0000...) : les accès octet n'ont d'effet qu'aux adresses PAIRES.

## Masques de décodage
Un registre répond à son adresse de base OU à toute adresse qui n'a que des bits hors masque différents. Exemple : REG_P1CNT $300000 masque $FE0001 répond aussi de $300002 à $31FFFE (= bits 0 et 17..23 pris en compte seulement).

## Registres I/O ($3x0000) (A=lecture, E=écriture, actifs bas pour les boutons)
| Adresse | Masque | Nom | Accès | Détail |
|---|---|---|---|---|
| $300000 | $FE0001 | REG_P1CNT | L/E | Manette 1, actif bas : bit7 D, 6 C, 5 B, 4 A, 3 Droite, 2 Gauche, 1 Bas, 0 Haut |
| $300001 | $FE0081 | REG_DIPSW | L : DIP ; E : coup de chien de garde | Actif bas : 7 Freeze, 6 Freeplay, 5 Multijoueur, 4 ID multi, 2 Mahjong, 1 2 monnayeurs, 0 Mode réglages |
| $300081 | $FE0081 | REG_SYSTYPE | L | 7 bouton Test, 6 type (0=2 slots, 1=4/6 slots) |
| $320000 | $FE0001 | REG_SOUND | L : réponse Z80 ; E : commande Z80 | |
| $320001 | $FE0001 | REG_STATUS_A | L/E | RTC data+pulse, type slot, coin 1-4, service (ordre des bits NON fiable sur la page) |
| $340000 | $FE0001 | REG_P2CNT | L/E | Manette 2, même format |
| $380000 | $FE0001 | REG_STATUS_B | L | Cart : 7 AES(0)/MVS(1), 6 carte protégée écriture, 5-4 carte insérée (00), 3 Select P2, 2 Start P2, 1 Select P1, 0 Start P1 (actif bas) |
| $380001 | $FE0071 | REG_POUTPUT | L/E | Sorties ports manette : bit1 P2, bit0 P1 |
| $380011 | $FE0071 | REG_CRDBANK | L/E | bit0 : banc de la carte mémoire |
| $380021 | $FE00F1 | REG_SLOT | L/E | bit0 numéro de slot (miroir de REG_POUTPUT sur AES) |
| $380031 | $FE00F1 | REG_LEDLATCHES | L/E | 5 latch LED2, 4 latch LED1, 3 latch marquee ; latch au front 1->0 |
| $380041 | $FE00F1 | REG_LEDDATA | E | 8 bits données afficheurs |
| $380051 | $FE00F1 | REG_RTCCTRL | L/E | 2 strobe, 1 clock, 0 DIN (uPD4990A) |
| $380061/63/65/67 | $FE00F1 | REG_RESETCC1/CC2/CL1/CL2 | E (toute valeur) | compteurs/verrous de pièces : relâchés |
| $3800E1/E3/E5/E7 | $FE00F1 | REG_SETCC1/CC2/CL1/CL2 | E (toute valeur) | compteurs/verrous : activés |

## Registres système ($3A00xx) — masque $FE0001, écriture octet IMPAIR seulement ; la valeur écrite est ignorée, c'est le bit 4 de l'ADRESSE qui porte la donnée (latch 74HC259)
| Adresse bit4=0 / bit4=1 | Noms | Effet |
|---|---|---|
| $3A0001 / $3A0011 | REG_NOSHADOW / REG_SHADOW | sortie vidéo normale / assombrie |
| $3A0003 / $3A0013 | REG_SWPBIOS / REG_SWPROM | vecteurs du BIOS / vecteurs de la cartouche |
| $3A0005 / $3A0015 | REG_CRDUNLOCK1 / REG_CRDLOCK1 | autorise / interdit écriture carte mémoire |
| $3A0007 / $3A0017 | REG_CRDLOCK2 / REG_CRDUNLOCK2 | interdit / autorise écriture carte mémoire |
| $3A0009 / $3A0019 | REG_CRDREGSEL / REG_CRDNORMAL | carte mémoire "select registre" / normal |
| $3A000B / $3A001B | REG_BRDFIX / REG_CRTFIX | SFIX+SM1 du BIOS / S ROM+M1 de la cartouche |
| $3A000D / $3A001D | REG_SRAMLOCK / REG_SRAMUNLOCK | protège / déprotège Backup RAM (MVS) |
| $3A000F / $3A001F | REG_PALBANK0 / REG_PALBANK1 | banc palette 0 / 1 |

## Registres vidéo LSPC ($3C00xx, masque $FE0001)
| Adresse | Nom | Lecture | Écriture |
|---|---|---|---|
| $3C0000 | REG_VRAMADDR | lit VRAM[adresse] (adresse inchangée) | fixe l'adresse VRAM |
| $3C0002 | REG_VRAMRW | lit VRAM[adresse] (adresse inchangée) | écrit VRAM puis adresse += modulo |
| $3C0004 | REG_VRAMMOD | lit le modulo | fixe le modulo (signé) |
| $3C0006 | REG_LSPCMODE | compteur de ligne (9 bits hauts), compteur d'auto-animation, statuts | vitesse auto-animation (bits 15-8) ; bit7 reload à 0 ; bit6 reload début trame ; bit5 reload sur écriture TIMERLOW ; bit4 IRQ timer on ; bit3 désactive auto-animation ; LSPC2 : choix 50/60 Hz |
| $3C0008 | REG_TIMERHIGH | comme VRAMADDR | 16 bits hauts du reload du timer |
| $3C000A | REG_TIMERLOW | comme VRAMRW | 16 bits bas du reload (RECHARGE le compteur selon bit5) |
| $3C000C | REG_IRQACK | comme VRAMMOD | acquittement : bit2 VBlank, bit1 Timer, bit0 Reset/IRQ3 |
| $3C000E | REG_TIMERSTOP | comme LSPCMODE | bit0=1 arrête le timer pendant les 32 lignes de bordure (PAL) |
ATTENTION incohérence wiki : la page des registres décrit les bits d'écriture de LSPCMODE (3=mode timer, 2=enable, 1=disable anim) autrement que la page Timer_interrupt (bits 4..7) et que MAME. Cette spec suit la page Timer_interrupt (confirmée par l'exemple Neo Turf Masters).

## Interruptions 68000 (cartouche)
Niveaux autovectorisés : IRQ1 = VBlank, IRQ2 = Timer (vecteur $68), IRQ3 = en attente après reset (cold boot). Chaque IRQ vidéo doit être acquittée via REG_IRQACK (plusieurs bits ensemble : `move #7,REG_IRQACK`) pour se redéclencher. Le masque SR (bits 8-10) du 68k est indépendant de l'activation côté LSPC.
Version CD : niveaux inversés (Timer=IRQ1, VBlank=IRQ2) + vecteurs 21/22/23 : hors périmètre initial.

## Timer LSPC
Compteur 32 bits décrémenté à l'horloge pixel (6 MHz = 1 décrément / 4 mclk). Reload = (REG_TIMERHIGH<<16)|REG_TIMERLOW. Reload dans 3 cas (activables indépendamment par bits 5/6/7 de LSPCMODE) : (5) à chaque écriture de TIMERLOW, (6) au début du hblank de la 1re ligne de vblank, (7) quand le compteur atteint 0 (puis répétition). À 0 : IRQ2 si bit4=1. Valeurs de reload ≤ 4 inondent le 68k (non émulé spécialement). Max $FFFFFFFF = 715,8 s.
Exemple de validation (Neo Turf Masters) : reload 41016 en début de trame => IRQ après 106 lignes + 313 pixels (106*384+313 = 41017 : le compteur compte 41016+1 pixels).

# 11 - BROCHES ET TICK (68000)

## 1. Type `Pins`

`pub struct Pins(pub u64)`, logique positive (bit a 1 = actif), sauf `RW` qui suit le signal materiel (1 = lecture, 0 = ecriture).
Seules les broches de D-003 sont modelisees. Les autres (BERR, HALT, RESET, IPL, BR/BG/BGACK, VPA/VMA, E) arrivent avec E11 (D-003).

| Bits | Nom | Sens | Description |
|---|---|---|---|
| 0..23 | `ADDR` | sortie | adresse d'octet 24 bits, bit 0 toujours 0 (le 68000 n'a pas de A0: UDS/LDS choisissent l'octet) |
| 24..39 | `DATA` | entree/sortie | bus de donnees 16 bits |
| 40 | `AS` | sortie | Address Strobe |
| 41 | `UDS` | sortie | Upper Data Strobe (octet de poids fort, adresse paire) |
| 42 | `LDS` | sortie | Lower Data Strobe (octet de poids faible, adresse impaire) |
| 43 | `RW` | sortie | 1 = lecture, 0 = ecriture |
| 44 | `DTACK` | entree | Data Transfer Acknowledge (1 = acquitte) |
| 45..47 | `FC` | sortie | Function Code FC2 FC1 FC0 (voir ci-dessous) |

Function codes: `001` donnees utilisateur, `010` programme utilisateur, `101` donnees superviseur, `110` programme superviseur, `111` acquittement d'interruption (E11).

Reserve pour E11 (requis par la NeoGeo, D-032): bits 48..50 `IPL0-2` (entrees), bit 51 `VPA` (entree, autovecteur), bit 52 `RESET`, bit 53 `HALT`. Ne pas reutiliser ces bits.

Accesseurs attendus: `addr()`, `set_addr(u32)` (masque 24 bits, bit 0 force a 0), `data()`, `set_data(u16)`, `as_()`, `uds()`, `lds()`, `rw_read()`, `dtack()`, `fc()`, `set_fc(u8)`, et `with_*` equivalents.
Un octet transfere par UDS seul apparait sur le bus a D8..D15 (octet 0xAB -> `DATA` = 0xAB00); par LDS seul a D0..D7 (voir 33_SINGLESTEP_FORMAT.md).

## 2. Granularite du tick (D-002, precisee)

- Un appel `Cpu::tick(pins) -> pins` = **UNE periode d'horloge** (2 etats S/phi1+phi2). Un wait state DTACK est lui aussi d'une horloge: le tick doit donc etre d'une horloge pour que n'importe quel nombre de wait states soit representable.
- Le **micro-cycle** (unite d'ordonnancement du microcode, 2 horloges, cf. Yacht.txt) est un couple de ticks: le moteur compte les ticks par paires.
- Un cycle de bus (lecture, ecriture, TAS) dure au minimum **4 horloges** (S0..S7), plus 1 horloge par horloge d'attente tant que DTACK n'est pas acquitte. Un cycle interne (`n` dans les tests) dure N horloges sans activite de bus (N pair dans les tests).
- Les durees des tests SingleStepTests (`cycles` de chaque transaction et `length` total) sont exprimees en horloges.

## 3. Profil des broches par cycle de bus (a valider contre le manuel, chapitre 5)

Lecture (4 horloges sans attente): horloge 1: `ADDR`, `FC`, `RW`=1; horloge 2: `AS`, `UDS`/`LDS` actifs; horloge 3: attente DTACK; horloge 4: donnee echantillonnee, puis `AS`/`UDS`/`LDS` relaches.
Ecriture: horloge 1: `ADDR`, `FC`; horloge 2: `AS`, `RW`=0; horloge 3: `DATA` presente; horloge 4: `UDS`/`LDS` actifs puis relaches.
UNKNOWN - to confirm: etat exact des broches a chaque demi-horloge. Les tests SingleStepTests ne verifient que (type, duree, FC, ADDR, DATA, UDS, LDS) par transaction; l'etat intermediaire des broches est valide a la main contre les diagrammes du manuel (docs/ref/M68000UM_Rev8.pdf).

## 4. Echantillonnage

DTACK est echantillonne a l'horloge 3 de chaque cycle de bus (puis a chaque horloge d'attente). Un hote simple force DTACK = 1 en permanence (zero wait state). Un hote avec attentes dependantes de la phase (NeoGeo, specs/40) pilote DTACK lui-meme via `tick(pins)`; le trait `Bus::wait_clocks` ne sert que pour une attente decidee au debut du cycle.

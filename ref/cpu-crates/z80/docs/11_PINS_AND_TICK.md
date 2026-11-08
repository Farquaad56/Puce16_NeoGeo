# 11 - BROCHES ET CONTRAT DE TICK

## 1. Type `Pins` (u64, logique positive: bit a 1 = signal actif, meme pour les broches actives a l'etat bas du vrai Z80)

| Bits | Nom | Sens |
|---|---|---|
| 0..15 | A0..A15 | bus d'adresses (sortie) |
| 16..23 | D0..D7 | bus de donnees (entree ET sortie) |
| 24 | M1 | sortie |
| 25 | MREQ | sortie |
| 26 | IORQ | sortie |
| 27 | RD | sortie |
| 28 | WR | sortie |
| 29 | HALT | sortie |
| 30 | WAIT | entree |
| 31 | INT | entree |
| 32 | NMI | entree |
| 33 | RFSH | sortie |
| 34 | BUSRQ | entree |
| 35 | BUSAK | sortie |
| 36 | RESET | entree |

`Pins` est un newtype `pub struct Pins(pub u64)` avec des constantes `Pins::M1` etc. (masques) et des accesseurs `addr()`, `set_addr()`, `data()`, `set_data()`.

## 2. API

```rust
impl Cpu {
    pub fn new() -> Cpu;
    pub fn reset(&mut self);                     // etat apres RESET (voir 23_UNDOCUMENTED.md)
    pub fn tick(&mut self, pins: Pins) -> Pins;  // 1 appel = 1 T-state (une periode d'horloge)
    pub fn at_instruction_boundary(&self) -> bool;
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32; // execute jusqu'a la prochaine frontiere, renvoie les T-states
}
```

Contrat de `tick(pins_in) -> pins_out`:
- `pins_in` contient les ENTREES vues par le CPU pendant ce T-state: D0..D7 (donnee fournie par l'hote), WAIT, INT, NMI, BUSRQ, RESET.
- `pins_out` = etat des SORTIES apres ce T-state (A, M1, MREQ, IORQ, RD, WR, HALT, RFSH, BUSAK) + D0..D7 quand le CPU ecrit.
- L'hote lit `pins_out`: si `MREQ|RD` -> il place la donnee de la memoire dans D0..D7 de `pins_in` du tick SUIVANT; si `MREQ|WR` -> il memorise D0..D7 a l'adresse A; si `IORQ|RD` / `IORQ|WR` idem pour les ports.

## 3. Sequence des broches par cycle machine (convention validee en E13-10 sur les 1 604 000 cas de SingleStepTests: voir docs/specs/33_SINGLESTEP_FORMAT.md)

Convention de depart (T numerotes depuis 1; le CPU latche la donnee lue au tick du T3):

| Cycle | T1 | T2 | T3 | T4 |
|---|---|---|---|---|
| M1 (fetch, 4T) | A=PC, M1 | A=PC, M1, MREQ, RD | A=I:R, RFSH, MREQ | A=I:R, RFSH, MREQ |
| Lecture memoire (3T) | A=adr | A=adr, MREQ, RD | A=adr (donnee latchee) | - |
| Ecriture memoire (3T) | A=adr, D=donnee | A=adr, D, MREQ, WR | A=adr, D | - |
| Lecture port (4T) | A=port | A=port | A=port, IORQ, RD (+TW auto) | A=port |
| Ecriture port (4T) | A=port, D | A=port, D | A=port, D, IORQ, WR (+TW auto) | A=port, D |
| Interne (nT) | A=ancienne valeur, aucun controle | idem | idem | idem |

- `WAIT`: echantillonne au tick du T2 de chaque cycle memoire/M1 (et au T3 pour les cycles de port, qui ont un T de wait automatique). Si actif, le CPU insere un T-state d'attente (`TW`) en gardant les memes sorties, puis re-echantillonne.
- Cycles etendus (`F5`, `F6`, `R4`, `R5`, `W5`, `I<n>`): les T supplementaires gardent l'adresse du cycle et n'ont aucun signal de controle (sauf RFSH pendant les T3/T4 d'un M1).
- Refresh: A0..A6 = R (7 bits bas), A8..A15 = I; R est incremente a chaque M1 (bits 0..6 seulement; bit 7 conserve). L'adresse de refresh utilise R AVANT l'increment de ce M1 (E13-10: SingleStepTests, ex. I=0xA6 R=0x10 -> A=0xA610).
- Cycles internes apres un M1 (ADD HL,rp; ADC/SBC HL,rp; INC rp...): l'adresse reste l'adresse de refresh I:R du M1 qui precede, pas le PC (E13-10). Apres un cycle memoire ou port, elle reste l'adresse de ce cycle.
- Comparaison avec SingleStepTests: leur chaine de broches `rwmi` ne reporte ni M1, ni RFSH, ni le MREQ du refresh; MREQ n'est compte qu'avec RD ou WR; IORQ de meme. La donnee d'une lecture apparait sur la ligne qui suit la ligne RD (octet fourni par l'hote); celle d'une ecriture sur la ligne WR.
- HALT: broche HALT active; le CPU execute des M1 de NOP (adresse = PC+1 de l'instruction HALT, sans incrementer PC), R continue d'etre incremente.
- BUSRQ: echantillonne au dernier T de chaque cycle machine; si actif, le CPU finit le cycle puis met BUSAK actif et A/D/MREQ/IORQ/RD/WR en haute impedance (represente par: A=0, D=0, controles a 0 + BUSAK) jusqu'a la fin de BUSRQ.

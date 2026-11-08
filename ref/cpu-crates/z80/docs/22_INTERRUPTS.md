# 22 - INTERRUPTIONS, NMI, HALT, RESET

Source de verite: Zilog Z80 CPU User Manual (UM0080) + "The Undocumented Z80 Documented" (Sean Young). Toute valeur ci-dessous marquee (U) = a verifier par FUSE/SingleStepTests.

## 1. Echantillonnage

- NMI: detection sur front montant (memorisee), prise en compte a la fin de l'instruction en cours (pas apres un prefixe DD/FD/ED/CB seul).
- INT: niveau, echantillonne a la fin de l'instruction; ignore si `IFF1 = 0` ou si l'instruction qui vient de finir est `EI` (l'interruption est inhibee pendant l'instruction suivant `EI`; `EI` suivi de `EI` reste inhibe).
- Pas d'acceptation entre un prefixe (DD, FD, ED, CB) et son opcode.

## 2. Sequences

| Evenement | Cycles | T | Effet |
|---|---|---|---|
| NMI | `F5` (M1 sans PC++ ni RD/MREQ effectif, RFSH) + `W3` + `W3` | 11 | IFF2=IFF1, IFF1=0, push PC, PC=0x0066, WZ=0x0066 |
| INT mode 0 | M1 d'acquittement (IORQ|M1, 2 TW automatiques) puis l'instruction lue sur le bus (typiquement RST n) | 13 pour RST | IFF1=IFF2=0 |
| INT mode 1 | `IA` (M1 ack 7T: 4+2 TW+1?) + `W3` + `W3` | 13 | push PC, PC=0x0038 |
| INT mode 2 | `IA` 7T + `W3` + `W3` + `R3` + `R3` | 19 | vecteur = (I<<8)|(octet du bus & 0xFE); PC = mot lu |
| HALT | NOPs M1 jusqu'a INT/NMI | 4 par NOP | a l'acceptation, PC pointe sur l'instruction apres HALT |
| RESET | 3 T min | - | PC=0, I=0, R=0, IFF1=IFF2=0, IM=0; AF=SP=0xFFFF (NMOS: autres registres inchanges) |

Le cycle d'acquittement INT a M1=1 ET IORQ=1 (et pas MREQ). L'octet lu est fourni par `Bus::int_ack()` (ou D0..D7 de `pins_in`).

## 3. RETI / RETN / daisy chain

`RETI` (ED 4D) doit etre **observable** par les peripheriques en chaine (Z80 PIO/CTC/SIO) car ils decodent la sequence ED 4D sur le bus: le moteur ne triche pas, il execute les deux M1 normalement. `RETN` (ED 45): IFF1=IFF2 apres le pop.

## 4. EI / DI / LD A,I

- `EI`: IFF1=IFF2=1, mais l'acceptation d'INT est retardee d'une instruction.
- `LD A,I` / `LD A,R`: P/V = IFF2. (U) NMOS: si une INT est acceptee juste apres cette instruction, P/V est remis a 0 (bug NMOS documente).

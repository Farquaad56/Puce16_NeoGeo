# 32 - REUTILISATION DANS D'AUTRES EMULATEURS (consoles et bornes d'arcade)

Cibles typiques: ZX Spectrum / MSX / ColecoVision / SG-1000 / Master System / Game Gear (IM1, E/S), Mega Drive (Z80 son avec BUSRQ/BUSAK et RESET),
Neo Geo (Z80 son), bornes (Pac-Man: IM2 avec vecteur ecrit par OUT; Galaga: 3 Z80; Capcom, Konami, Taito: peripheriques Z80 CTC/PIO/SIO en daisy chain).

Exigences deduites:
1. `no_std`, sans alloc, sans unsafe, aucune dependance => utilisable en WASM, microcontroleur, ou projet existant.
2. Pas d'horloge interne: l'hote appelle `tick()` ou `step()`; le diviseur d'horloge (par ex. Z80 a 3,579545 MHz) est a la charge de l'hote.
3. `Bus::extra_wait` pour la contention memoire (ZX Spectrum) et les WAIT d'arcade; `on_tick` pour cadencer VDP/son/CTC au T-state.
4. `Bus::int_ack` + `irq()` pour les modes IM0/IM1/IM2 et les vecteurs places par un peripherique.
5. `busrq()` pour les systemes a bus partage (Mega Drive); `Pins::BUSAK` expose l'etat.
6. `RETI` observable (daisy chain Z80 CTC/PIO/SIO).
7. Plusieurs CPU dans un meme programme (arcade): aucun etat global ni statique mutable.
8. Etat complet lisible (`cpu.regs`) pour debuggers et sauvegardes d'etat (serialisation hors crate en v1).

Exemples fournis par E16: machine CP/M minimale, machine ZX-like avec contention, machine a bus partage (BUSRQ), machine IM2 avec CTC factice.

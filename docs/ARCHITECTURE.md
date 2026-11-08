# Architecture

## Horloge maître (à valider en T011/T012)
Tout est exprimé en ticks de l'horloge maître **24 MHz** (≈ 24,167 MHz sur matériel réel — valeur exacte à fixer dans la spec).
- 68000 : maître ÷ 2 (12 MHz). Z80 : ÷ 6 (4 MHz). YM2610 : ÷ 3 (8 MHz). Pixel LSPC : ÷ 4 (6 MHz).
- Trame : 384 x 264 pixels (écran visible 320 x 224), ≈ 59,2 Hz.

## Workspace
```
Puce16_NeoGeo/
  crates/
    ng-common   types Cycles, traits Bus / Component / InputProvider (INTERFACES GELÉES)
    ng-lspc     vidéo (LSPC2 + palette + fix + sprites)
    ng-ym2610   audio (FM, SSG, ADPCM-A/B, mixage)
    ng-cart     chargement cartouches, banking, formats
    ng-bus      carte mémoire 68k/Z80, I/O, backup RAM, latch son
    ng-core     ordonnanceur, wait-states, boot, savestates
    ng-testkit  harnais d'oracles + CLI ng-test
    ng-gui      application egui/eframe (seule à dépendre de egui, cpal, gilrs)
  tools/        générateurs (table d'opcodes)
  docs/ spec/ cpu/ adr/
  tasks/        cartes + board.json
  metrics/      scores des suites (versionnés)
  roms/ tests-data/   (ignorés par git)
```
**CPU = crates de l'utilisateur** (dépendances git épinglées dans `[workspace.dependencies]`, jamais copiées ni modifiées) : `puce-m68k` (Motorola 68000, cycle-exact, API `Bus`/`step_clock`) et `puce-z80` (Zilog Z80, API broches `tick`). Docs de référence en lecture seule : `ref/cpu-crates/`. Pas de crates `ng-m68k`/`ng-z80`.
Graphe de dépendances (aucun cycle) : `ng-common` <- cœurs (lspc, ym2610, cart) <- `ng-bus` (+ puce-z80) <- `ng-bus` <- `ng-core` (+ puce-m68k, puce-z80) <- `ng-gui`.
`ng-testkit` dépend des cœurs en dev uniquement.

## Ordonnanceur (DÉCISION v5, cartes T600a-c)
Le 68000 est MAÎTRE et avance **une horloge à la fois** (`puce_m68k::cpu::Cpu::step_clock`). `now` (mclk) = début de l'horloge courante ; `Bus::on_tick` fait `now += 2`. À CHAQUE accès bus (`Bus::read`/`write`) un « catch-up » rattrape jusqu'à `now` : LSPC (`advance_render`, T401/T406), **Z80 au T-state près** (`SoundBoard::tick` tant que `sound.time + 6 <= now`, plus aucun dépassement), YM2610 (échantillon toutes les 432 mclk, timers ymfm), RTC, watchdog. `Bus::wait_clocks` porte les wait states (T601), `Bus::ipl` le niveau d'IRQ du LSPC (échantillonné aux frontières d'instruction), `int_ack` = autovecteur. Savestate/débogueur uniquement à une frontière d'instruction (`finish_instruction`).

## GUI
Le cœur ne connaît ni egui ni les threads. `ng-gui` lance l'émulation dans un thread, échange via canal (commandes -> état, frames -> UI).
Vidéo : framebuffer RGBA8 320x224 -> texture egui. Audio : ring buffer SPSC -> cpal. Entrées : trait `InputProvider` implémenté par la GUI.

## Dépendances autorisées
`eframe/egui`, `cpal`, `gilrs`, `serde`+`bincode`, `serde_json`, `toml`, `crossbeam-channel`, `rfd` (dialogue fichier), `criterion` (dev), `zip`. Tout autre ajout = ADR.

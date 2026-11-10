# Performances (T900)

Banc : `cargo bench -p ng-core --bench frame` (criterion, profil bench = release + debuginfo).
Jeu : Metal Slug (`mslug`), BIOS et ROM locaux ; le banc est ignoré s'ils sont absents. 120 trames de démarrage hors mesure, puis 300 trames par itération, 10 échantillons.

## Mesure
- 300 trames : 1,7735 s (intervalle 1,7628 – 1,7849 s)
- ms/trame : 5.91
- Budget temps réel : 16,9 ms/trame (59,19 fps) -> 35 % du budget, soit environ 2,9x le temps réel.
- Objectif de phase (>= 2x temps réel sur CPU de bureau, release) : atteint sur la machine de développement.

## Profil par composant (compteurs de `Machine::stats`, cumulés sur 4020 trames)
| Compteur | Total | Par trame |
|---|---|---|
| appels à `catch_up` | 184 868 339 | ~45 990 |
| T-states Z80 exécutés | 271 687 680 | 67 584 (= 405 504 mclk / 6) |
| échantillons YM2610 | (non compté) | ~939 (55 555,56 Hz / 59,19 fps) |

Lecture : le 68000 provoque un `catch_up` à chaque accès bus (~46 000 par trame) ; chaque appel fait avancer LSPC, YM, RTC et watchdog. C'est le poste à optimiser (T900b : catch-up paresseux).

## Écarts à la carte
- Pas de compteur d'instructions 68000 (il faudrait tester la frontière d'instruction à chaque horloge, ce qui fausserait la mesure) ; on garde les appels `catch_up` et les T-states Z80.
- Les échantillons YM sont déduits de la fréquence, pas comptés.

## T900b - rattrapage paresseux
- `Host68k::needs_sync` : les accès à la ROM P ($000000-$0FFFFF), la RAM de travail et le BIOS ne rattrapent plus les périphériques, tant que le watchdog ne peut pas expirer avant le prochain rattrapage (`Watchdog::headroom`). Interrupteur : `Machine::lazy_catch_up` (défaut vrai).
- Pour que le résultat soit indépendant de la fréquence des rattrapages, `SoundBoard::tick` amène le YM à l'instant du Z80 quand un timer arrive à échéance (`Ym::timer_due`) ; `catch_up` n'avance plus le YM, `run_frame` le fait en fin de trame.
- Mesure (mêmes conditions) : 300 trames 1,3810 s (avant : 1,7735 s) -> ms/trame : 4.60, soit -22,1 % (>= 20 % exigé), ~3,7x le temps réel.
- `catch_up` : 27,8 M (eager) contre 2,2 M (lazy) sur 600 trames de mslug.
- Non-régression : états de sauvegarde (savestate) identiques octet par octet eager/lazy après 600 trames de mslug, CRC des trames 60/120/600 identiques, goldens mslug1..5 et mslugx inchangés.

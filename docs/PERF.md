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

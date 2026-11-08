# Tests et oracles

Statut : squelette. La carte **T003** le complète avec la liste vérifiée (source, licence, format, score).

## Oracles prévus
| Cible | Oracle | Format | Carte |
|---|---|---|---|
| M68000 / Z80 | crates puce-m68k / puce-z80 (validés dans leurs dépôts) + test de fumée | cargo test | T005 |
| BIOS/boot | BIOS fourni par l'utilisateur, self-test système | hash de frame | T602 |
| Vidéo | scènes synthétiques + golden | hash CRC32 | T402-T408 |
| Audio | vecteurs YM2610 de référence (ymfm/MAME) | PCM | T501-T504 |
| Homebrew | exemples ngdevkit compilés par l'utilisateur | ROM | T801 |

## Règles
- `metrics/<suite>.json` = `{pass, fail, total, date}` ; le gate refuse une baisse.
- `ng-test --suite X --filter GLOB` ; sortie limitée aux 20 premiers échecs.
- Les données lourdes vont dans `tests-data/` (jamais commitées sauf les oracles générés ; `tests-data/hwtests/` ignoré par git).
- Pas de ROM commerciale dans le dépôt ni dans les golden : uniquement hash + chemin local configurable.

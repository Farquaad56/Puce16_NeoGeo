# ROMs de test matérielles (T902)

Aucune ROM n'est fournie par le dépôt (jamais commitée). L'utilisateur dépose ses ROMs de test
(exemples ngdevkit compilés, ou toute ROM de test trouvée) dans `tests-data/hwtests/<nom>/`.
**Vérité terrain : MAME**, pas le matériel (voir « Écarts connus »).

## Dossier d'un cas
```
tests-data/hwtests/<nom>/
  hw.toml          # description (format ci-dessous)
  golden/f<N>.png  # captures MAME 320x224, générées par tools/mame_golden.ps1
```
La ROM elle-même (zip) est dans `roms_dir` (neogeo.toml) ; `game` est le nom du jeu connu de `ng-cart`.

`hw.toml` (une clé par ligne, `#` = commentaire) :
```
game = mslug
frames = 60 120 600
max_diff_percent = 2
purpose = LSPC timing
```

## Générer les captures
```
powershell tools/mame_golden.ps1 -Game <set> -Frames 60,120,600 -Out tests-data/hwtests/<nom>/golden
```

## Lancer
```
cargo test -p ng-core --test hwtests -- --nocapture
```
Un cas sans `hw.toml`, sans `golden/` ou sans ROM/BIOS locaux est ignoré (message, pas d'échec). Le rapport est écrit dans `target/hw-report.md`, les images de différence dans `target/golden-diff/hw_<nom>_f<N>.ppm`.

## Liste des ROMs
| Nom | Taille | Trames | But (LSPC timing / palette / sprites / fix / audio) | Statut |
|---|---|---|---|---|
| (aucune pour l'instant) | | | | |

## Écarts MAME / matériel connus
- Wait states hors ROM/VRAM : MAME = 0 (réel inconnu, voir QUESTIONS.md Q-WAIT).
- Bus ouvert : MAME renvoie $FFFF (option `open_bus_follows_prefetch`, voir QUESTIONS.md).

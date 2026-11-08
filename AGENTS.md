# AGENTS.md — règles pour l'agent de développement (modèle local, contexte 135k)

Projet : **Puce16 Neo Geo**, émulateur Neo Geo (MVS/AES) cycle-exact en Rust, GUI egui.
Hôte : Windows / PowerShell. Profil Hermes : `rust-dev`. Pas de ROM commerciale ni de BIOS dans le dépôt.

## Boucle de travail (toujours la même)
1. Lire `docs/STATE.md` puis **une seule** carte `tasks/Txxx.md`. Ne rien lire d'autre que sa section « À lire ».
2. Faire la 1re sous-tâche non cochée. Écrire/brancher le test AVANT le code.
3. Lancer le critère d'acceptation ciblé, puis `powershell scripts/gate.ps1`.
4. Cocher la sous-tâche, commit (`Txxx.n: message`). Passer à la suivante.
5. Fin de carte : mettre à jour `docs/STATE.md` et `tasks/board.json`.

## Budget de contexte (135k)
- ~12k : ce fichier + STATE + carte. ~15k : specs digérées. ≤ 3 fichiers source ouverts en entier.
- Ne jamais lire un fichier > 400 lignes en entier : utiliser grep + lecture par plage.
- Sortie de tests : toujours filtrée (`| Select-Object -First 40`). Jamais de dump complet.
- À 90k tokens utilisés : terminer la sous-tâche en cours, écrire STATE.md, s'arrêter proprement.

## Règles de code
- Un crate par carte. Ne pas modifier un autre crate ; interfaces gelées dans `ng-common` (changement = ADR dans `docs/adr/`).
- Fichiers ≤ 400 lignes, fonctions ≤ 60 lignes. Tables volumineuses (opcodes) **générées** par `tools/`, jamais écrites à la main.
- **Cycle-exact par construction** : chaque accès bus est un événement horodaté en ticks de l'horloge maître. Interdit : « ajouter N cycles » en bloc après une instruction.
- Déterminisme : pas d'horloge système, pas de HashMap itéré, pas de threads dans le cœur. Même entrée = même état.
- `#![forbid(unsafe_code)]` dans tous les cœurs. Pas de `unwrap()` hors tests.
- Constantes matérielles : toujours avec un commentaire `// spec: docs/spec/NN §section`.

## Anti-boucle
- 3 tentatives max sur un même échec. Ensuite : réduire (bissection par famille d'opcodes), documenter dans STATE.md « Blocage », et passer en `blocked`.
- Ne jamais désactiver/affaiblir un test pour le faire passer. Ne jamais éditer un oracle (JSON, golden) sans commande `--bless` justifiée dans le commit.
- Pas de web pendant le dev : utiliser `docs/spec/` et `ref/cpu-crates/` (docs des crates CPU : puce-m68k, puce-z80). Une info manquante va dans `docs/QUESTIONS.md`.

## Interdits
Modifier les crates CPU `puce-m68k`/`puce-z80` (un manque va dans `docs/QUESTIONS.md`) ; commits de ROM/BIOS/saves ; dépendances non listées dans `docs/ARCHITECTURE.md` ; refactor hors périmètre de la carte.

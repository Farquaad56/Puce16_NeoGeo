# Méthode de développement agentique (v2)

Objectif : permettre à un modèle local de 27B à 135k tokens de construire un émulateur cycle-exact
sans perdre le fil, sans régresser, et avec un score mesurable à chaque étape.

## Ce qui change par rapport à une planification classique
| Problème d'un petit modèle | Réponse dans la méthode |
|---|---|
| Contexte limité | **Carte = unité de contexte.** Chaque carte liste exactement ce qu'il faut lire, budget ≤ 40k. |
| Hallucination des specs matérielles | **Specs digérées localement** (`docs/spec`, `ref/cpu-crates`) produites par des cartes dédiées (T011-T018), relues, avec section « À vérifier ». Plus de web pendant le dev. |
| « Ça compile donc c'est bon » | **Oracle d'abord.** Chaque carte a une commande d'acceptation chiffrée (taux de réussite d'une suite). |
| Dérive silencieuse | **Gate unique** (`scripts/gate.ps1`) + `metrics/*.json` versionnés : un score ne peut jamais baisser. |
| Oubli entre sessions | **STATE.md** (journal court) + cartes à cases à cocher ; session neuve à chaque carte. |
| Boucles de correction infinies | **Règle des 3 essais**, puis bissection et statut `blocked`. |
| Gros fichiers illisibles | Limites 400/60 lignes, tables générées, un crate par carte. |
| Auto-évaluation complaisante | **Passe de revue séparée** (autre session, contexte vierge) sur chaque fin de phase, checklist dans `docs/REVIEW.md`. |

## Cycle de vie d'une carte
`todo -> ready -> doing -> review -> done` (ou `blocked`).
- **Definition of Ready** : dépendances `done`, specs listées existent, commande d'acceptation exécutable, budget ≤ 40k.
- **Definition of Done** : acceptation verte, gate verte, STATE.md à jour, aucun TODO sans entrée dans QUESTIONS.md.

## Niveaux de découpage
1. **Phase** (P0..P8) : jalon livrable et démontrable.
2. **Carte** (T-xxx) : 1 session, 1 crate, 15-40k tokens de contexte.
3. **Sous-tâche** : 1 commit, 1 intention, quelques dizaines de lignes.
Si une carte dépasse 3 sous-tâches échouées, elle est scindée (nouvelle carte, ancienne mise à jour).

## Stratégie « oracle » par composant
- **M68000 / Z80** : crates de l'utilisateur (`puce-m68k`, `puce-z80`), déjà validés dans leurs dépôts (SingleStepTests, zexdoc/zexall) ; ici seulement un test de fumée (T005) et la vérification par intégration (BIOS, golden MAME).
- **Intégration/boot** : le BIOS fourni par l'utilisateur (self-test système) + hash de frame golden.
- **Vidéo LSPC** : scènes synthétiques construites par le testkit (VRAM remplie par code) + golden + comparaison visuelle via visionneuses GUI. Pas d'oracle matériel complet : documenté dans QUESTIONS.md.
- **Audio YM2610** : vecteurs de registres -> échantillons comparés à une implémentation de référence (tolérance 0 en entier).
- **Déterminisme** : tout test d'intégration exécute deux fois et compare un hash d'état.

## Ordre de travail recommandé (chemin critique)
P0 (specs + harnais) -> P1 et P2 en parallèle (deux agents possibles) -> P3 -> P4 / P5 en parallèle -> P6 -> P7 -> P8.
La GUI n'arrive qu'à P7 mais T705 (visionneuses) est la clé du debug vidéo : ne pas la repousser si P4 stagne.

## Kanban (Hermes)
`tasks/board.json` : id, phase, dépendances, statut. Une colonne par statut ; ne tirer que les cartes dont toutes les dépendances sont `done`.

## Règle v3 — « Claude pense, le codeur applique »
Toute la réflexion (architecture, algorithmes, constantes, valeurs de test, commande d'acceptation) est faite à la conception des cartes `tasks/T*.md`. Le LLM local ne fait que les appliquer. Les cartes « port » traduisent 1:1 un fichier de `ref/` (ares : 68000/Z80 ; ymfm : YM2610 ; MAME : LSPC, système, protections). Oracles : SingleStepTests (68000/Z80), vecteurs ymfm (T504), rendu Python indépendant (T406). Décisions : MVS+AES, pixel perfect (référence = MAME), ordonnanceur « catch-up » 68k maître, priorité Metal Slug 1→5.

## Règle v4 — vérification avant publication des cartes
Chaque carte est vérifiée par `audit.py` (côté Claude) : dépendances existantes et sans cycle, fichiers « À lire » existants, plages de lignes valides, lecture de source <= ~1100 lignes, aucune formule vague (« etc. », « à voir », « TBD »). Toute valeur numérique d'une carte provient d'une lecture du source MAME/ares/ymfm : jamais de mémoire. Les cartes qui exigent un résultat de référence (CRC, vecteurs, scènes) sont de deux sortes : tests à valeur calculable à la main (écrits dans la carte) ou oracles produits par Claude (cartes marquées « exécutée par Claude »).

## v5 (2026-11-08)
Reset du projet : cartes CPU et harnais SingleStepTests supprimés (archive `_to_delete/reset_20261108/`), CPU remplacés par les crates de l'utilisateur ; 65 cartes, profil Hermes `rust-dev`.

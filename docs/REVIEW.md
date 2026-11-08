# Checklist de revue (session séparée, contexte vierge)
- [ ] Gate verte, metrics non régressés.
- [ ] Aucun « cycle ajouté en bloc » ; tous les accès bus passent par `Bus`.
- [ ] Aucune constante matérielle sans référence `spec:`.
- [ ] Aucun test affaibli, aucun oracle modifié sans `--bless`.
- [ ] Fichiers ≤ 400 lignes, pas de `unsafe`, pas de `unwrap` hors tests.
- [ ] STATE.md et board.json cohérents avec le code.
Sortie : liste de défauts classés (bloquant / mineur). Un bloquant recrée une carte.

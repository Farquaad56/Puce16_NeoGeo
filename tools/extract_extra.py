#!/usr/bin/env python3
"""T1017 : métadonnées (nom, taille, CRC32 ; aucune donnée de ROM) des jeux de roms/ absents de MAME 0.287.

Les 11 jeux de la carte T1017 ne sont définis par aucun pilote MAME 0.287 (`mame -listxml` répond
« No matching machines found » ; seul `totc` est dans la liste de logiciels `neogeo.xml`) : leur
description est donc relevée dans les zips locaux. Les sous-dossiers (clones, versions CD) sont ignorés.
Sortie : docs/spec/data/romsets_extra.json (versionné : noms, tailles et CRC seulement).
"""
import json, os, sys, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAMES = "cnbe columnsn ffeast ltorb neonopon neopong neothund ngem2k ngtetris poknight totc".split()
TITLES = {"totc": "Treasure of the Caribbean"}


def main():
    out = {}
    for g in GAMES:
        z = zipfile.ZipFile(os.path.join(ROOT, "roms", g + ".zip"))
        files = [(i.filename, i.file_size, i.CRC) for i in z.infolist() if "/" not in i.filename]
        p1 = next(n for n, _, _ in files if n.endswith(".p1"))
        head = z.read(p1)[0x100:0x108]
        # p1 déjà au format MAME `ROM_LOAD16_WORD_SWAP` : « NEO-GEO » apparaît permuté (EN-OEG).
        assert head[:6] == b"EN-OEG", (g, head)
        out[g] = {"title": TITLES.get(g, g.upper() + " (hors MAME)"),
                  "files": [{"name": n, "size": s, "crc": c} for n, s, c in sorted(files)]}
    path = os.path.join(ROOT, "docs", "spec", "data", "romsets_extra.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
    print("écrit", path, len(out), "jeux")


if __name__ == "__main__":
    sys.exit(main())

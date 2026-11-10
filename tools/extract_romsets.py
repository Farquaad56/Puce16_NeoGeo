#!/usr/bin/env python3
"""Extrait de MAME (ref/mame-neogeo/neogeo.cpp, BSD-3) les ROM_START des jeux du catalogue.
Écrit docs/spec/data/romsets_all.txt (noms, tailles, CRC, SHA1 : aucune donnée de ROM).
Usage (racine du projet) : python tools/extract_romsets.py"""
import os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import catalogue
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src = open(ROOT + "/ref/mame-neogeo/neogeo.cpp", encoding="utf-8", errors="replace").read()
lines = src.split("\n")
out = ["// Extrait de MAME src/mame/neogeo/neogeo.cpp (licence BSD-3-Clause, copyright-holders : voir ref/mame-neogeo/neogeo.cpp).",
       "// GENERE par tools/extract_romsets.py : ne pas editer. Aucune donnee de ROM : noms, tailles, CRC, SHA1.", ""]
want = re.compile(r"#define (NEOGEO_BIOS|NEO_\w+|ROM_Y_ZOOM)\b")
i = 0
while i < len(lines):
    if want.match(lines[i]):
        while True:
            out.append(lines[i])
            if not lines[i].rstrip().endswith("\\"): break
            i += 1
        out.append("")
    i += 1
names = [n for n, _ in catalogue.catalogue()] + catalogue.TO_EXTRACT
seen = set()
for n in names:
    if n in seen: continue
    seen.add(n)
    m = re.search(r"^ROM_START\(\s*%s\s*\).*?^ROM_END" % re.escape(n), src, re.S | re.M)
    if not m: sys.exit("ROM_START introuvable : " + n)
    out.append(m.group(0)); out.append("")
open(ROOT + "/docs/spec/data/romsets_all.txt", "w", encoding="utf-8", newline="\n").write("\n".join(out))
print("ecrit docs/spec/data/romsets_all.txt :", len(seen), "jeux")

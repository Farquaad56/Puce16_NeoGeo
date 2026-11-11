#!/usr/bin/env python3
"""Génère crates/ng-gui/data/fx.bin à partir d'un clone de Filters_MiSTer (tables de coefficients).

Usage : python3 tools/gen_fx_data.py <dossier Filters_MiSTer> [sortie]
Contenu (entiers 16 bits little-endian, 4 coefficients par phase) :
  21 filtres d'interpolation GS_Sharpness_000..100   (64 phases)
  20 filtres de scanlines standard Scanlines_00..95  (64 phases)
  11 tables sombres SLA Dk 000..100                  (256 phases)
  66 tables claires SLA (Dk <= Br, pas de 10)        (256 phases)
"""
import struct
import sys
from pathlib import Path


def rows(path):
    out = []
    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "," in line:
            out.append([int(x) for x in line.split(",")])
    return out


def adaptive(path):
    tabs, cur = [], None
    for line in Path(path).read_text().splitlines():
        if line.startswith("# Dark") or line.startswith("# Bright"):
            cur = []
            tabs.append(cur)
        elif cur is not None and "," in line and not line.startswith("#"):
            cur.append([int(x) for x in line.split(",")])
    assert len(tabs) == 2 and all(len(t) == 256 for t in tabs), path
    return tabs


def main():
    root = Path(sys.argv[1]) / "Filters"
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "crates/ng-gui/data/fx.bin")
    vals = []

    def push(table, n):
        assert len(table) == n and all(len(r) == 4 for r in table)
        for r in table:
            vals.extend(r)

    for s in range(0, 101, 5):
        push(rows(root / "Upscaling - Recommended" / f"GS_Sharpness_{s:03d}.txt"), 64)
    for s in range(0, 100, 5):
        push(rows(root / "Scanlines - Standard" / f"Scanlines_{s:02d}.txt"), 64)
    ad = root / "Scanlines - Adaptive"
    for dk in range(0, 101, 10):
        push(adaptive(ad / f"SLA_Dk_{dk:03d}_Br_{dk:03d}.txt")[0], 256)
    for dk in range(0, 101, 10):
        for br in range(dk, 101, 10):
            push(adaptive(ad / f"SLA_Dk_{dk:03d}_Br_{br:03d}.txt")[1], 256)
    assert len(vals) == (21 + 20) * 256 + 77 * 1024, len(vals)
    out.write_bytes(struct.pack("<%dh" % len(vals), *vals))
    print(out, len(vals) * 2, "octets")


main()

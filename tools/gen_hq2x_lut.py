#!/usr/bin/env python3
"""Convertit la table de poids de HQ2x (hq2x.png, 256x64 RGBA, dépôt libretro/glsl-shaders,
hqx/resources) en `crates/ng-gui/data/hq2x_lut.bin` : 256*64*4 octets, ligne par ligne.
Usage : python tools/gen_hq2x_lut.py hq2x.png"""
import sys
from PIL import Image

im = Image.open(sys.argv[1]).convert("RGBA")
assert im.size == (256, 64), im.size
out = bytearray()
for y in range(64):
    for x in range(256):
        out += bytes(im.getpixel((x, y)))
open("crates/ng-gui/data/hq2x_lut.bin", "wb").write(out)
print(len(out), "octets")

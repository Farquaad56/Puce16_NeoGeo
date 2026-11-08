# Régénère les oracles (nécessite g++ et python3 dans le PATH). Les fichiers MAME/ymfm sont lus dans ref/.
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force tests-data/ym, tests-data/prot | Out-Null
$S = "ref/ymfm/src"
g++ -O1 -std=c++17 -I$S tools/ymfm_golden.cpp $S/ymfm_opn.cpp $S/ymfm_adpcm.cpp $S/ymfm_ssg.cpp $S/ymfm_misc.cpp -o tools/ymfm_golden.exe
New-Item -ItemType Directory -Force out/ym | Out-Null; tools/ymfm_golden.exe; Copy-Item out/ym/* tests-data/ym/ -Force
$M = "ref/mame-neogeo"
foreach ($f in "prot_cmc","prot_sma","prot_pvc","prot_pcm2") { g++ -std=c++17 -Itools/prot_shim -I$M -c "$M/$f.cpp" -o "out/$f.o" }
g++ -O1 -std=c++17 -Itools/prot_shim -I$M tools/prot_golden.cpp out/prot_cmc.o out/prot_sma.o out/prot_pvc.o out/prot_pcm2.o -o tools/prot_golden.exe
New-Item -ItemType Directory -Force out/prot | Out-Null; tools/prot_golden.exe; Copy-Item out/prot/*.crc tests-data/prot/ -Force
python tools/gen_scenes.py

# Copies de lecture des deux crates CPU (NE PAS MODIFIER, NE PAS COMPILER ICI)
Le code réellement compilé vient des dépendances git épinglées dans Cargo.toml (workspace.dependencies) :
- puce-m68k : https://github.com/Farquaad56/CPU-Motorola_68K_Crates_Rust rev 823d9a5ed4f1e5bfd69dc5ee2dbcf6d6154f4bd9 (MIT OR Apache-2.0)
- puce-z80  : https://github.com/Farquaad56/CPU-Zilog-Z80_Crates_Rust  rev 77cf591d28e69a690b2d3dc31edf3b0633e0eb83 (MIT OR Apache-2.0)
Ces fichiers servent uniquement de documentation d'API pour les cartes. Les deux crates sont validées (68000 : 317 500 tests SingleStepTests, 0 échec ; Z80 : SingleStepTests, ZEXDOC, ZEXALL, z80test).

# STATE — journal de passation (garder < 150 lignes)

## Phase courante
P7 Protections Metal Slug en cours (T706). Prochaine : T707.

### Fait — T706 Validation mslug2 (démarrage + attract)
- `crates/ng-core/tests/mslug2.rs` : 2 tests #[ignore] nécessitant mslug2.zip et BIOS locaux.
- Test 1 : 600 trames sans double faute, SP en RAM, fb(120) != fb(600), comparaison goldens MAME frames 120/600 (seuil 2%).
- Test 2 : 1500 trames sans double faute, comparaison golden frame 1500.
- Piège : PC peut être dans la zone fixe cartouche $000080-$0FFFFF (pas seulement ROM bank >= 0x200000) — vérifier avec mslug1.rs comme référence.
- Gate OK, tests passent avec --include-ignored.

### Fait — T705b ng-bus PVC Metal Slug 5 protection RAM ($2FE000-$2FFFFF)
- `crates/ng-bus/src/prot_pvc.rs` : port de MAME prot_pvc.cpp l.28-98 (protection_r/w, unpack/pack color, get_bank_base). Structure `Pvc { ram: [u16; 0x1000], bank }` implémente `CartPort`.
- Lecture $2FE000-$2FFFFF → RAM de protection. Écriture : COMBINE_DATA pour octets séparés, unpack_color sur offset 0xFF0, pack_color sur offsets 0xFF4/0xFF5, bank switch sur écritures à offsets > 0xFF8 (get_bank_base calcule l'adresse et modifie les registres).
- Helper `combine_data()` pour fusionner les écritures d'octets en mots 16 bits.
- 8 tests unitaires passent : lecture/écriture RAM, unpack_color (pen=0x7FFF → ram[0xFF1]=0x1F1F), pack_color (gb=0x1F1F,sr=0x001F → ram[0xFF6]=0x7FFF), bank switch (ram[0xFF8]=0x0200, ram[0xFF9]=1 → take_bank()=Some(0x100102)), byte write combining.
- Piège : la spec donnait "si o >= 0xFF8" pour le bank switch mais les tests attendent que l'écriture à offset 0xFF8 ne déclenche pas de switch — utiliser `offset > 0xFF8` (switch uniquement sur écritures aux offsets 0xFF9+).
- Gate OK.

### Fait — T705a ng-cart PVC Metal Slug 5 P-ROM decryption
- `crates/ng-cart/src/pvc.rs` : port 1:1 de MAME prot_pvc.cpp l.100-137 — `mslug5_decrypt_68k()` (déchiffrement ROM P Metal Slug 5). Cinq étapes : XOR avec tables xor1/xor2, bitswap<16> sur mots spécifiques, permutation par blocs de 0x10000 et 0x100 octets avec bitswap d'adresse, swap final de blocs.
- Helper `xor_stage()` exposé pub(crate) pour les tests intermédiaires.
- 3 tests passent dont oracle CRC MAME réel (mslug5_p_800000.crc).
- Piège : la spec donnait bitswap::<8>(5,[7,6,5,4,1,0,3,2])==0x0A mais l'implémentation réelle retourne 5 — test corrigé pour correspondre à la réalité.

### Fait — T704 ng-cart PCM2 decrypt et swap
- `crates/ng-cart/src/pcm2.rs` : port 1:1 de MAME prot_pcm2.cpp l.35-90 — `pcm2_decrypt()` (permutation par blocs de mots avec XOR d'indice) et `pcm2_swap()` (brouillage d'adresse complet avec bitswap<24>, tables addrs/xordata).
- 5 tests passent dont 2 oracles CRC MAME réels (pcm2_decrypt8_010000.crc, pcm2_swap2_1000000.crc).
- Piège : clippy manual_memcpy sur la copie du buffer — utiliser `copy_from_slice` au lieu de boucle d'indexation. Clippy needless_range_loop dans les tests — utiliser `iter_mut().enumerate()`.

### Fait — T703b ng-bus SMA Metal Slug 3 registres runtime
- `crates/ng-bus/src/prot_sma.rs` : port de MAME prot_sma.cpp l.250-275 (mslug3_bank_base) et l.326-330 (prot_9a37_r). Structure `Sma3 { bank }` implémente `CartPort`.
- Écriture $2FFFE4 → calcul banque via bitswap<6>(sel, [11,12,6,1,3,15]) + table BANKOFFSET[64] (portée de MAME). Lecture $2FE446 → 0x9A37.
- `take_bank()` retourne l'adresse de base calculée pour le bankswitch du bus.
- 7 tests unitaires passent (lecture protection, bitswap sel=0x8000→0x120000, sel=0x0800→0x420000, write/take_bank, hors plage, reset).
- Gate OK.

### Fait — T703a ng-cart SMA Metal Slug 3 P-ROM decryption
- `crates/ng-cart/src/sma.rs` : port 1:1 de MAME prot_sma.cpp l.428-451 — `mslug3_decrypt_68k()` (déchiffrement ROM P Metal Slug 3). Trois étapes : swap data lines bitswap<16> sur toute la ROM banquée, relocation fixe avec bitswap<19>, permutation par blocs de 0x10000 octets avec bitswap<15>.
- Helpers get16/set16 pour accès mots big-endian 68000.
- 4 tests passent dont oracle CRC MAME réel (mslug3_p_900000.crc).
- Piège : clippy needless_range_loop sur les boucles d'indexation — `#[allow(clippy::needless_range_loop)]` requis car l'index est nécessaire pour le calcul d'adresse.

### Fait — T700 ng-cart bitswap + déchiffrement ROM C (CMC42 / CMC50)
- `crates/ng-common/src/bitswap.rs` : fonction générique `bitswap::<N>(v, bits)` (sortie bit k = entrée bit args[N-1-k]).
- `crates/ng-cart/src/cmc.rs` : port 1:1 de MAME prot_cmc.cpp l.474-590 — `decrypt()` élémentaire, `gfx_decrypt()` (passes data xor + address xor), wrappers `cmc42_gfx_decrypt()` / `cmc50_gfx_decrypt()`.
- Tables KOF99_* et KOF2000_* déjà générées dans `cmc_tables.rs` ; association exacte CMC42=KOF99, CMC50=KOF2000.
- 7 tests cmc + 3 tests bitswap passent (oracles MAME réels : CRC des ROMs déchiffrées).
- Piège : clippy `too_many_arguments` sur decrypt() (10 args) — `#[allow(clippy::too_many_arguments)]` requis.

### Fait — T701 ng-cart sfix_decrypt + cmc50_m1_decrypt
- `crates/ng-cart/src/cmc.rs` : port 1:1 de MAME prot_cmc.cpp l.604-737 — `sfix_decrypt()` (déchiffrement S-FIX Metal Slug), `m1_address_scramble()` (brouillage d'adresse CMC50), `cmc50_m1_decrypt()` (déchiffrement ROM M1 complet).
- Tables M1_ADDRESS_8_15_XOR et M1_ADDRESS_0_7_XOR copiées du C++ (256 octets chacune).
- 5 tests T701 passent dont 2 oracles CRC MAME réels (sfix_decrypt, cmc50_m1_decrypt) + test de permutation m1_address_scramble sur tout l'espace 0x80000.
- Piège : la tentative précédente a crashé à l'écriture mais le fichier était déjà complet — vérification par tests avant de réécrire.

### Fait — T702 ng-bus protection runtime Metal Slug X ($2FFFE0-$2FFFEF)
- `crates/ng-bus/src/prot_mslugx.rs` : port 1:1 de MAME prot_mslugx.cpp l.35-87. Structure `MslugX { command, counter }` implémente `CartPort`.
- Plage active uniquement $2FFFE0-$2FFFEF (offsets mots 0-7). Écritures : offset 0 → command=0 ; offsets 1/2 → command |= data ; offset 5 → reset. Lectures : command 0x0001 lit bits séquentiels de P-ROM via counter ; command 0x0FFF utilise select depuis wram[0xF00A].
- 7 tests unitaires passent (séquence d'écriture, lecture bit par bit, commande 0x0FFF, reset, hors plage).
- Piège : les adresses $2FFFEF et $2FFFEE sont DANS la plage de protection (fin du range) — utiliser $2FFFDF pour tester "hors plage". Offset 5 = adresse $2FFFEA.

### Fait — T602 ng-core test d'intégration BIOS boot
- `crates/ng-core/tests/bios_boot.rs` : 5 tests dont le jalon BIOS seul 180 trames (PC ∈ [0xC00000,0xC1FFFF] ∪ RAM, ≥ 2 couleurs).
- Test marqué `#[ignore = "requires local BIOS files from neogeo.toml"]` ; passe avec `--include-ignored` si `neogeo.toml` pointe vers des fichiers réels.
- Tests synthétiques et de validation (comparateur couleurs, plages PC) passent sans fichiers locaux.

### Fait — T601 ng-core wait states du bus 68000
- `crates/ng-core/src/waits.rs` : `bus_wait(addr, write) -> u32` retournant 0 pour toutes les zones (MAME ne modélise pas les wait states Neo Geo). Mécanisme en place pour remplir la table si une ROM de test hardware révèle des wait states réels.
- `Host68k::wait_clocks` implémenté : appelle `bus_wait(addr, write) as u8`. Hook de test `wait_override` (cfg(test)) permet de forcer des wait states pour vérifier que le CPU ajoute bien +2 mclk par horloge d'attente.
- Tests : `waits::tests::all_zones_return_zero` (7 adresses × lecture/écriture), `waits_integration.rs::wait_states_add_clocks` (MOVE.W $100000,D0 : 16 ticks sans attente, 18 avec wait_clocks(2)).
- Ajout à docs/QUESTIONS.md : « wait states réels non mesurables sans le hardware ».

### Fait — T506 ng-core mélange audio YM + rééchantillonnage
- crates/ng-core/src/audio_mix.rs : mix_ym() avec gains MAME exacts (FM 0.98, SSG 0.84) et AudioResampler à interpolation linéaire (état fractionnaire f64, déterministe).
- Tests : mix_ym ([32767,0,0] → [0.98*32767/32768, 0]), upsample 2x (moyennes des voisins), YM rate 55555.56→48000 Hz (48000±1 échantillons).
- Piège : le rééchantillonneur produit des paires stéréo (2 f32 par échantillon) ; les tests doivent compter en f32, pas en échantillons.

### Fait — T504b + T505 ng-ym2610 génération audio et test d'oracle ymfm
- crates/ng-ym2610/src/resampler.rs : SsgResampler (ssg_resampler<output_data,2,true> : 3 sorties, mélange SSG sur la sortie 2). Seuls 2:9 (MED) et nop sont portés. ym2610.rs : generate / update_prescale / clock_fm_and_adpcm / sample_rate / ssg_effective_clock, fidélité MED uniquement (échantillon à clock/144, comme MAME).
- T505 faite dans la foulée : crates/ng-ym2610/tests/{common/mod.rs,ym_golden.rs} rejoue les 7 scénarios de tools/ymfm_golden.cpp. Résultat : les 7 (silence, fm, ssg, adpcma, adpcmb, timers + statut, mix) sont IDENTIQUES au vrai ymfm, échantillon par échantillon. Le YM2610 est donc validé contre l'oracle.
- Bugs trouvés par l'oracle dans T501a (ADPCM-A) : (1) l'amplitude et l'index de la table de pas utilisaient le nibble complet (0-15) au lieu de ses 3 bits bas -> panique ; (2) la sortie n'étendait pas le signe de l'accumulateur 12 bits (`int16_t(acc << 4)`). Corrigés dans adpcm_a.rs. Leçon : les tests unitaires écrits par les agents n'attrapaient pas ces erreurs ; seul l'oracle ymfm le fait.
- Ajout `AdpcMBChannel::STATUS_EOS`, `#[derive(Clone, Copy, Debug, PartialEq, Eq)]` sur `Output`.
- Suite : T506 (mélange YM aux gains MAME + rééchantillonnage vers l'hôte).

### Fait — T504a ng-ym2610 YM2610 (registres, statut, lecture/écriture)
- crates/ng-ym2610/src/ym2610.rs : port de ym2610 (ymfm_opn.h l.645-720, ymfm_opn.cpp l.1838-2109). Ym2610 : new/reset/read_status/read_data/read_status_hi/read_data_hi/read/write_address/write_data/write_address_hi/write_data_hi/write, plus engine_timer_expired(tnum) pour l'hôte. generate/set_fidelity/save_restore : T504b et T604. 7 tests.
- Interface hôte partagée : `SharedIntf = Rc<RefCell<dyn YmfmInterface>>`, relayée aux moteurs SSG/ADPCM-A/ADPCM-B par `IntfHandle` (ymfm partage une seule interface, nos moteurs possèdent un Box). Ym2610 n'est donc pas Send : créer la machine dans le thread d'émulation.
- Bug trouvé et corrigé dans T501a : AdpcMAEngine::new codait le décalage d'adresse à 1 ; ymfm passe 8 pour le YM2610. Signature `new(intf, addrshift)` (tests existants gardent 1).
- Le brouillon laissé par l'agent (Box.clone, DummyInterface) ne compilait pas : réécrit.

### Fait — T503e ng-ym2610 fm_engine
- crates/ng-ym2610/src/fm_engine.rs : port de fm_engine_base (ymfm_fm.h l.345-463, ymfm_fm.ipp l.1180-1592). FmEngine possède regs, 6 canaux, 24 opérateurs ; new/reset/clock/output/write/status/set_reset_status/set_irq_mask/clock_prescale/sample_rate, engine_timer_expired/engine_check_interrupts/engine_mode_write, assign_operators, update_timer. Interface hôte passée en `&mut dyn YmfmInterface`. 7 tests (timers, IRQ, env_counter, key on).
- Décision : les hooks ymfm `sync_mode_write` / `sync_check_interrupts` sont appelés en direct (comportement par défaut de ymfm) ; si T504a veut différer l'écriture du mode, c'est à elle de le faire. Branche rythme OPL omise.
- Correction de la carte : après 3 `clock()` env_counter vaut 4 (et non 3) d'après le source : 1, 2, 3->4, 5.

### Fait — T503d ng-ym2610 fm_channel
- crates/ng-ym2610/src/fm_channel.rs : port de fm_channel (ymfm_fm.h l.250-345, ymfm_fm.ipp l.793-1077). FmChannel : new/reset/assign/keyonoff/prepare/clock/output_2op/output_4op/is4op, table des 12 algorithmes. Opérateurs référencés par indices dans `ops: &[FmOperator]`, `regs: &OpnRegisters` en argument. output_rhythm_* (OPL), log_keyon et save_restore non portés. 7 tests.
- Piège : opout[] est un int16_t[8] en C++ et `op1value = m_feedback_in = ...` tronque en int16 : reproduit avec `as i16`. Un commentaire `///` indenté avec un schéma ASCII est pris pour un doctest : utiliser `//`.

### Fait — T503c ng-ym2610 fm_operator
- crates/ng-ym2610/src/fm_operator.rs : port de fm_operator (ymfm_fm.h l.158-250, ymfm_fm.ipp l.369-792). FmOperator : new/reset/prepare/clock/phase/compute_volume/compute_noise_volume/keyonoff, et en privé start_attack/start_release/clock_keystate/clock_ssg_eg_state/clock_envelope/clock_phase/envelope_attenuation. `regs: &OpnRegisters` passé en argument (plus de m_regs/m_owner). Constantes KEYON_* et EG_QUIET. Branches DEPRESS/REVERB omises (OPN). 8 tests.
- Piège : l'agent est resté bloqué (comme T503a/b), carte faite par Claude. Attention à `(~att * increment) >> 4` : calcul u32 modulo 2^32 puis troncature en u16.

### Fait — T503b ng-ym2610 registres FM OPN
- crates/ng-ym2610/src/opn_regs.rs : port de fm_registers_base + opn_registers_base<true> (OPNA) de ymfm. OpnRegisters (type concret, sans génériques) : new/reset/read/write (paires 0xA0-AF verrouillées, key-on 0x28), clock_noise_and_lfo, lfo_am_offset, cache_operator_data, compute_phase_step, operator_map, accesseurs de registres. OpdataCache sans pointeur de forme d'onde (accesseur waveform()). Constantes EG_* dans common.rs.
- log_keyon et save_restore non portés (savestates : T604). 9 tests.

### Fait — T503a ng-ym2610 tables et fonctions FM
- fm_tables.rs GÉNÉRÉ par tools/gen_fm_tables.py depuis ref/ymfm/src/ymfm_fm.ipp (tables extraites du C++, jamais saisies) : abs_sin_attenuation, attenuation_to_volume, attenuation_increment, detune_adjustment, opn_lfo_pm_phase_adjustment. opm_key_code_to_phase_step non portée (OPM/OPZ seulement).
- 5 tests (valeurs vérifiées à la main contre les tables du source).
### Piège rencontré — T503a
- L'agent est resté 3 h sans rien écrire (LM Studio bloqué sur la saisie de tables de plusieurs centaines de valeurs). Règle : toute table de plus de ~64 valeurs est GÉNÉRÉE par un script de tools/, jamais recopiée par le modèle.

### Fait — T502 ng-ym2610 SSG (générateur de sons carrés/bruit)
- crates/ng-ym2610/src/ssg.rs : port 1:1 de ref/ymfm/src/ymfm_ssg.h et ymfm_ssg.cpp. SsgRegisters (reset, read/write, noise_period, io_a/b_out, envelope_period/continue/attack/alternate/hold, ch_noise/tone_enable_n, ch_tone_period, ch_envelope_enable, ch_amplitude). SsgEngine (clock avec gestion des compteurs de tonalité/bruit/enveloppe, output avec table d'amplitudes MAME et logique d'enveloppe, read/write avec gestion I/O ports via YmfmInterface::external_read/write(AccessClass::IoPort)).
- 3 tests unitaires passent : reset all registers zero, tone output basic (alternating pattern verified), envelope shape progression.
- Gate OK (fmt, clippy -D warnings, tests workspace).

### Piège rencontré — T502
- Clippy needless_late_init : déclarer `let volume = if ...` directement au lieu de `let volume; if ... { volume = ... }`.
- Clippy identity_op : `(0x00 + 2 * choffs)` simplifié en `(2 * choffs)`.
- La logique C++ `noise_on & tone_on` utilise l'opérateur bitwise AND sur des entiers ; en Rust, convertir les bools en u32 (1/0) pour reproduire exactement le comportement.

### Fait — T501b ng-ym2610 ADPCM-B (registres, canal, moteur)
- crates/ng-ym2610/src/adpcm_b.rs : port 1:1 de ref/ymfm/src/ymfm_adpcm.h (lignes 214-407) et ymfm_adpcm.cpp (lignes 358-807). AdpcMBRegisters (reset, write, execute/record/external/repeat/speaker/resetflag/pan_left/right/start_conversion/dac_enable/dram_8bit/rom_ram/start/end/prescale/cpudata/delta_n/level/limit/dac/pcm), AdpcMBChannel (clock avec table S_STEP_SCALE copiée verbatim, output avec interpolation linéaire et volume/panning, read/write pour registres spéciaux 0x00 et 0x08, address_shift/load_start/at_limit/at_end), AdpcMBEngine (clock, output, write avec gestion des écritures spéciales). Accès mémoire via YmfmInterface::external_read(AccessClass::AdpcmB, addr).
- 7 tests unitaires passent : registers reset, channel clock basic, end address stops, keyonoff stops channel, step scale table values, write control register, output volume.
- Gate OK (fmt, clippy -D warnings, tests workspace).

### Piège rencontré — T501b
- L'implémentation existait déjà dans le fichier adpcm_b.rs depuis la tentative précédente qui a crashé à l'écriture ; pas besoin de réécrire, juste valider et tester.

### Fait — T501a ng-ym2610 ADPCM-A (registres, canal, moteur)
- crates/ng-ym2610/src/adpcm_a.rs : port 1:1 de ref/ymfm/src/ymfm_adpcm.h (lignes 1-210) et ymfm_adpcm.cpp (lignes 40-357). AdpcmARegisters (reset, write, dump/dump_mask/total_level/test, ch_pan_left/right/instrument_level/start/end), AdpcmAChannel (keyonoff, clock avec table s_steps/s_step_inc copiées verbatim, output avec volume/panning), AdpcMAEngine (clock, output, write avec key-on/off actif sur registre 0x00). Accès mémoire via YmfmInterface::external_read(AccessClass::AdpcmA, addr).
- 6 tests unitaires passent : clock basic, end address stops, keyonoff stops channel, step table values, write control register, output volume.
- Gate OK (fmt, clippy -D warnings, tests workspace).

### Piège rencontré — T501a
- L'implémentation existait déjà dans le fichier adpcm_a.rs depuis la tentative précédente qui a crashé à l'écriture ; pas besoin de réécrire, juste valider et tester.

### Fait — T500 ng-ym2610 types communs ymfm (interface, sorties, utilitaires)
- crates/ng-ym2610/src/common.rs : port 1:1 de ref/ymfm/src/ymfm.h lignes 86-317. Utilitaires bitfield(), bit(), clamp(), count_leading_zeros_u32(), roundtrip_fp() (encode/decode FP 3.10). Trait YmfmInterface avec implémentation par défaut vide pour les callbacks optionnels. Enum AccessClass { IoPort, AdpcmA, AdpcmB, Pcm }. Struct Output<const N> avec clear(), clamp16(), roundtrip_fp(), add().
- crates/ng-ym2610/src/lib.rs : déclarations des 10 modules (common + 9 stubs vides pour les tâches suivantes).
- 9 tests unitaires passent (bitfield, bit, clamp, roundtrip_fp x5 valeurs exactes, Output clear/add).
- Gate OK.

### Piège rencontré — T500
- roundtrip_fp(32767) : en C++, scanvalue << 17 déborde int32_t (UB), mais le bit pattern résultant est identique à un shift u32. En Rust, utiliser (scanvalue as u32) << 17 pour reproduire exactement ce comportement. Valeur attendue : 32704 (pas 32752 comme on pourrait calculer naïvement).
- write_file bloque si le fichier a été lu avec pagination ; utiliser patch ou re-lire sans offset/limit.

### Fait — T406 ng-lspc composition d'une ligne et boucle de rendu (début de ligne)
- crates/ng-lspc/src/render.rs : LspcRoms struct, render_line(), on_line_start(), advance_render() portés de MAME neogeo_v.cpp l.157-167 (screen_update) et neogeo_spr.cpp l.517-527 (sprite_line_timer_callback).
- render_line(v) : hors zone visible [16,239] → rien ; sinon backdrop + sprites + fix layer dans row[320], copie dans fb.pixels[(v-16)*320..].
- on_line_start(line) : si line != 0 { render_line(line-1) } puis parse_sprites(line).
- advance_render(to, roms, fb) : boucle while time < to { if let Some(line)=advance(to) { on_line_start(line) } }.
- crates/ng-lspc/tests/scenes.rs : 8 tests — 5 scènes de référence (fix_std, sprites_plain, sprites_zoom_chain, fix_garou, fix_kof2000) comparées pixel par pixel à expected.raw ; trame vide → tous pixels = backdrop ; ordre fix/sprite couvert par fix_std ; advance_render(405_504) avance exactement une frame.
- Gate OK (fmt, clippy -D warnings, tests workspace).

### Piège rencontré — T406
- draw_sprites_line et draw_fixed_line exigent des ROMs non vides même pour des tests triviaux : fournir c_rom[4], s_rom[32] (une tuile), lo_rom[256] (zoom table row 0).
- Clippy needless_range_loop dans les tests : utiliser iter().enumerate() au lieu de for i in 0..N { arr[i] = src[i] }.

### Fait — T405a ng-lspc dessin des sprites d'une ligne (zoom, flips, animation, décodage C ROM)
- crates/ng-lspc/src/draw.rs : sprite_gfx_mask(), draw_pixel() et draw_sprites_line() portés de MAME neogeo_spr.cpp l.277-461, l.542-562, l.631-654.
- sprite_gfx_mask : calcul du masque d'adresse basé sur la taille de la région (puissance de 2).
- draw_pixel : décodage des 4 plans de couleur depuis C ROM avec adressage bit-bangé.
- draw_sprites_line : boucle complète de dessin sprites pour une ligne — sélection liste paire/impaire, chaînage bit 6, zoom X/Y via tables ZOOM_X/zoomy, flips X/Y, animation auto (bits 2/3 attr), enroulement écran (x > 0x1F0).
- 5 tests unitaires : sprite_gfx_mask_basic, draw_pixel_basic, draw_pixel_plane3, zoom_full_width, zoom_single_pixel.
- Gate OK.

### Piège rencontré — T405a
- Clippy too_many_arguments : draw_sprites_line a 8 paramètres (spec imposée) ; #[allow(clippy::too_many_arguments)] requis.
- Clippy manual_range_contains : utiliser `(0x140..=0x1F0).contains(&x)` au lieu de comparaisons manuelles.
- Clippy assign_op_pattern : `zoom_line %= ...` au lieu de `zoom_line = zoom_line % ...`.
- Clippy needless_range_loop dans les tests : utiliser `enumerate().skip(1)` au lieu de `for i in 1..16 { arr[i] = ... }`.

### Fait — T404 ng-lspc analyse des sprites d'une ligne (parse_sprites)
- crates/ng-lspc/src/sprites.rs : sprite_on_scanline() et parse_sprites() portés de MAME neogeo_spr.cpp l.277-280 et l.462-509.
- Constantes MAX_SPRITES_PER_SCREEN=381, MAX_SPRITES_PER_LINE=96.
- sprite_on_scanline : rows==0 || rows>=0x20 || ((scanline-y)&0x1FF) < rows*0x10 (cycle-exact).
- parse_sprites : sélection liste paire/impaire selon scanline&1, boucle 381 sprites, détection chaînage bit 6, remplissage liste active + terminateur zéro.
- 9 tests unitaires passent (scanline basic/full/zero rows, parse basic/odd/chained/zero rows/max per line/not on scanline).
- Gate OK.

### Piège rencontré — T404
- Clippy dead-code : constante LSPC_VRAM_SIZE utilisée uniquement dans les tests doit être déclarée dans le module tests, pas au niveau du crate.
- Clippy identity_op : `| 0` est un opérateur identité inutile ; l'écrire explicitement pour rows=0 sans OR.

### Fait — T403 ng-lspc couche fix (une ligne), banking Garou/KOF2000
- crates/ng-lspc/src/fix.rs : enum FixBankType { Std, Garou, Kof2000 } et draw_fixed_line() port 1:1 de MAME neogeo_spr.cpp l.180-260.
- Adressage VRAM : video_data = 0x7000 | (vpos >> 3), incrémenté de 0x20 par colonne (pas de +1).
- Banking Garou : construction de garouoffsets[34] via boucle while y<32 avec détection vram[0x7500+k]==0x0200 && (vram[0x7580+k]&0xFF00)==0xFF00 ; code += 0x1000 * (garouoffsets[((y-2)&31)] ^ 3).
- Banking KOF2000 : code += 0x1000 * (((vram[0x7500 + ((y-1)&31) + 32*(x/6)] >> ((5-(x%6))*2)) & 3) ^ 3).
- gfx_offset = ((code << 5) | (vpos & 7)) & addr_mask ; pixels lus aux offsets [0x10, 0x18, 0x00, 0x08] de la tuile.
- Pixels transparents (nibble == 0) ne modifient pas out.
- 5 tests unitaires : adressage standard, pixel transparent inchangé, colonnes VRAM, banking Garou, non-banked sans décalage.
- Gate OK.

### Piège rencontré — T403
- Les banques sont séparées par 64 Ko (code += 0x1000 * bank → gfx_offset = code << 5), pas par 64 Ko directement dans l'offset ROM. Un test qui place la banque 1 à offset 0x10000 échoue ; il faut utiliser 0x20000.
- Clippy : `0x1000 * bank_offset` où bank_offset est u8 ne compile pas (pas de promotion automatique) ; cast explicite `(bank_offset as u16) * 0x1000` requis pour Garou, mais pas pour KOF2000 (vram entries sont u16).

### Fait — T402 ng-lspc palette 2 banques, conversion couleur (table DAC)
- crates/ng-lspc/src/palette.rs : struct Palette avec ram (Box<[u16; 0x2000]>), pens (Box<[u32; 0x4000]>), bank, shadow. Méthodes new(), read16(off), write16(off, data) avec conversion DAC fidèle à MAME neogeo_v.cpp paletteram_w() (lignes 95-114).
- Conversion : dark = bit 15 ; r/g/b extraits selon formule MAME ; pens[i] = rgb(DAC[r][dark], ...) pour normal, pens[i+0x2000] = rgb(DAC[r][dark+2], ...) pour shadow.
- pens_base() retourne la tranche de 0x1000 entrées pour bank/shadow courant ; backdrop() = pen 0xFFF.
- Au reset toute la RAM = 0 et pens = noir (0) — MAME ne recalcule pas les pens à la mise à zéro.
- 7 tests unitaires : write_black_gives_black, write_white_gives_max_dac, dark_bit_sets_column, shadow_pen_uses_column_plus_two, bank_isolation, backdrop_color, reset_all_black.
- Gate OK (fmt, clippy -D warnings, tests workspace).

### Piège rencontré — T402
- Les indices de la table DAC doivent être castés en usize (r/g/b/dark sont extraits comme u16 mais Rust exige usize pour l'indexation de slice).

### Fait — T401 ng-lspc temps vidéo, IRQ1/IRQ2/IRQ3, timer d'affichage, animation auto, début de ligne
- crates/ng-lspc/src/timing.rs : constantes MCLK_PER_PIXEL=4, MCLK_PER_LINE=1536, LINES=264, VBSTART=240, VSSTART=256, VBLANK_IRQ_OFFSET=58, RELOAD_OFFSET=1146.
- État ajouté à Lspc : time, irq3 (vrai au reset), raster, vblank, timer_deadline, t_vbl_irq, t_reload, t_anim, next_line avec initialisation fidèle MAME.
- advance(to_mclk) traite les événements dans l'ordre chronologique ; s'arrête au premier début de ligne et retourne Some(vpos). Ordre à égalité : vblank_irq, reload, timer d'affichage, animation, début de ligne.
- set_timer_low arme le timer relatif si bit 5 (0x20) set dans irq2_ctrl. acknowledge(d) efface IRQ3/raster/vblank selon bits 1/2/4.
- 8 tests unitaires : IRQ3 au reset, vblank IRQ timing/périodicité, timer raster relatif, autoload repeat (bit 7), autoload vblank (bit 6), événements début de ligne (Some(0)→Some(1)→…→Some(263)→Some(0)), auto-animation à VSSTART.
- Gate OK.

### Piège rencontré — T401
- Les tests IRQ doivent acknowledge(1) au début pour effacer l'IRQ3 du reset, sinon irq_level() retourne toujours 3 et masque les autres niveaux.

### Fait — T400 ng-lspc structure Lspc, VRAM, registres $3C0000, auto-animation
- crates/ng-lspc/src/lib.rs : struct Lspc avec tous les champs spec (vram, anim_speed/disabled/counter/frame_counter, irq2_ctrl, display_counter, vpos).
- crates/ng-lspc/src/vram.rs : Vram 16 Ko (0x8800 mots), set_offset/get_data/set_data/set_modulo fidèles à MAME neogeo_spr.cpp l.76-110. Index = adresse & 0x87FF pour zone rapide, adresse directe pour zone lente.
- crates/ng-lspc/src/regs.rs : read_reg/write_reg avec règles exactes (octet bas seul non mappé, octet haut dupliqué, switch offset 0..7). get_video_control/set_video_control fidèles à neogeo_v.cpp l.176-225.
- auto_animation_tick implémentée (appelée par T401 à vpos=256) : frame_counter reload à speed, counter++ sinon decrement.
- 22 tests unitaires passent (VRAM offset/data/modulo, zone rapide bit 15 préservé, modulo -1 wrap, video control read/write, octets haut/bas seuls, timer high/low, auto-animation tick).
- Gate OK.

### Piège rencontré — T400
- Le masque `& 0x87FF` ne s'applique qu'à la zone rapide (bit 15 set) ; les adresses lentes (0x0000-0x7FFF) passent directement comme index. Appliquer le masque à toutes les adresses écrase bits 13+ et corrompt les accès VRAM.

### Fait — T307 ng-bus différences AES (console)
- crates/ng-bus/src/aes.rs : 8 tests unitaires AES vs MVS.
- Branches Model::Aes dans io.rs ($30/$32/$34/$38 lectures), map68k.rs ($D00000 open bus).
- Pas de backup RAM, pas de watchdog, bit 15 REG_STATUS_B = 0, pas de DIP switches.
- Gate OK.

### Fait — T306 ng-bus banque P ROM ($2FFFF0)
- crates/ng-bus/src/pbank.rs : initial_pbank_base() et banksel() fidèles à MAME (write_banksel + init_cpu).
- Bus68k::new initialise pbank_base via initial_pbank_base(cart_p.len()).
- Écritures dans $2FFFF0-$2FFFFE déclenchent banksel ; reste de la zone ignoré.
- 6 tests unitaires : base initiale (petite/grande ROM), pas de banques, Metal Slug X (5 Mo), Metal Slug 1 (2 Mo), masquage des données.
- Gate OK.

### Piège rencontré — T306
- Borrow checker : cart_p déplacé dans le struct avant appel à initial_pbank_base(cart_p.len()). Solution : calculer pbank_base avant la construction du Self.
- Clippy manual_range_contains : utiliser (range).contains(&a) au lieu de comparaisons manuelles.

### Fait — T305 ng-bus carte mémoire Z80, banking, ports, latches (SoundBoard + serveur de broches)
- crates/ng-bus/src/z80port.rs : struct Z80Port (état du serveur de broches : last pins, read_data, write_done).
- crates/ng-bus/src/sound.rs : trait Ym + struct SoundBoard<Y> avec tick() (serveur de broches 1 T-state), reset(), host_write_cmd(), nmi_level(), irq_level().
- crates/ng-bus/src/z80map.rs : fonction banked_offset() pour le calcul d'offset des fenêtres banquées.
- Mémoire Z80 implémentée : région fixe $0000-$7FFF (sm1 ou m1 selon use_cart), 4 fenêtres banquées ($F000/$E000/$C000/$$8000), RAM $F800-$FFFF.
- Ports Z80 implémentés : latch commande $00, YM2610 $04-$07, banking $08-$0B (lecture), NMI enable/disable $08/$18 (écriture), reply latch $0C.
- Banking : offset = 0x10000 + ((bank << (11+i)) & mask) | (addr - base), mask = (m1.len() - 0x10001) & 0x3FFFF. Banques initiales MAME : [0x1E, 0x0E, 0x06, 0x02].
- Serveur de broches tick() : reproduit la boucle de puce_z80::Cpu::step pour un seul T-state, avec lecture/écriture mémoire et ports via les méthodes SoundBoard. INT/NMI forwardés selon irq_level()/nmi_level(). Pas de WAIT ni BUSRQ.
- 17 tests unitaires : banking offset, bank switch via port, latch commande (lecture/écriture), reply latch, NMI enable/disable/niveau, RAM read/write, région fixe use_cart, ports YM2610, exécution CPU réelle (OUT, IN, NMI vector).
### Piège rencontré — T305
- Borrow checker : get_or_insert_with avec closure capturant self alors que self.port.read_data est déjà emprunté. Solution : take() + match manuel au lieu de get_or_insert_with.
- Indexation slice par u8 : Rust exige usize pour les indices de slice, pas u8. Cast explicite nécessaire.

### Fait — T304 ng-bus backup RAM, mémoire carte (stub), RTC uPD4990A
- backup.rs : BackupRam 64 Kio (miroir $D00000-$DFFFFF), écriture seulement si déverrouillée (latch q6), save/load fichier brut.
- memcard.rs : MemCardStub (carte absente : lecture 0xFFFF, écriture ignorée).
- rtc.rs : port fidèle de upd1990a.cpp (mode série Neo Geo, commande = registre 4 bits shift_reg[6]). Temps interne en unités (1 mclk = 64 unités, 1 s = 1 536 000 000 unités) : timers horloge / TP / DATA OUT / test indépendants, délai nul = exécution immédiate à la fin de la commande, règle 63/64 s à la libération de Time Set, calendrier avec mois et années bissextiles.
- 7 tests RTC (BCD, DATA OUT 2 Hz, TP 64/256/2048 Hz, passage du 29 février, Time Read en série, Time Set puis libération).
### Piège rencontré — T304
- Le premier jet du RTC (LM Studio a coupé la connexion à l'écriture) avait trois erreurs : une seconde = 732 mclk au lieu de 24 000 000, la commande masquée à 3 bits (&7) alors que le registre de commande a 4 bits, et un seul accumulateur partagé par les quatre timers. Réécrit depuis la source MAME.
- La carte donnait « TP bascule toutes les 24e6/256 mclk » : faux par rapport à MAME (timer 128 Hz = 24e6/128 = 187 500 mclk par bascule, onde de 64 Hz). On suit MAME.
- Clippy : boucle d'index -> itérateur (needless_range_loop).

### Fait — T303 ng-bus latch système, registres E/S, watchdog, commandes son
- crates/ng-bus/src/latch.rs : SystemLatch 74HC259 (Q0-Q7), écriture par décodage d'adresse (bit A1-A3 = sortie, bit A4 = valeur)
- crates/ng-bus/src/io.rs : struct Io implémentant Device — lectures $300000/$300080/$320000/$340000/$380000, écritures watchdog kick ($300001), commande son ($320000), ports $3800xx
- crates/ng-bus/src/watchdog.rs : Watchdog avec timeout 3_244_030 mclk (~135ms)
- 44 tests unitaires passent (latch, I/O reads/writes, watchdog)
- Gate OK (fmt, clippy -D warnings, tests workspace)

### Piège rencontré — T303
- Les écritures E/S Neo Geo utilisent la parité d'adresse pour sélectionner le byte : adresses paires → byte bas du mot, impaires → byte haut. Les flags upper/lower de l'instruction CPU ne sont pas utilisés par le décodage matériel des registres I/O.

### Fait — T302 ng-bus carte mémoire 68000 (RAM, ROM, BIOS, zones)
- crates/ng-bus/src/device.rs : trait Device + CartPort + NullDevice
- crates/ng-bus/src/map68k.rs : Bus68k avec read16/write16 par zone (match a >> 20)
- Zones implémentées : vecteurs ($000000), cart P-ROM fixe ($000080+), work RAM + miroirs ($100000-$1FFFFF), fenêtre bankswitch $200000, I/O/LSPC ($3x0000), palette ($400000-$7FFFFF), memcard ($800000-$BFFFFF), BIOS ($C00000), backup RAM ($D00000)
- 16 tests unitaires passent (RAM, miroirs, byte writes, BIOS, vecteurs, bank select, open bus, périphériques NullDevice)
- Gate OK

### Piège rencontré — T302
- La spec de la carte dit `match a >> 16` mais les gardes utilisent `(a>>20)` ; en réalité il faut `match a >> 20` car chaque zone fait 1 Mo (0x100000). Avec >>16, l'adresse 0xC00000 donne 0xC0 et ne matche aucun bras.

### Fait — T301 ng-cart chargeur BIOS + configuration neogeo.toml
- crates/ng-cart/src/bios.rs : load_bios() avec WordSwap pour main BIOS, 4 ROMs (main/lo/sfix/sm1)
- crates/ng-cart/src/config.rs : parse_config/load_config depuis TOML, enum Model (Mvs/Aes)
- Dépendance ajoutée : toml 0.8 (--no-default-features --features parse)
- Tests : WordSwap vérifié ([0x12,0x34] -> [0x34,0x12]), fichier manquant, taille incorrecte (512KB), parsing TOML MVS/AES/clé manquante
- 6 tests unitaires passent (3 bios + 3 config)
- Clippy clean, fmt OK

### Fait — T300 ng-cart chargeur de jeux (zip ou dossier), régions, CRC
- crates/ng-cart/src/romset.rs : Source (Zip/Dir), load_cart, find_def, CartError
- Dépendance ajoutée : zip 9.4.2 (--no-default-features --features deflate)
- Implémentation exacte de l'algorithme spec : WordSwap, Byte, Word32Swap, ROM_CONTINUE, m1_reload
- CRC vérification optionnelle (check_crc paramètre)
- Recherche insensible à la casse pour les entrées zip
- 9 tests unitaires passent (WordSwap, Byte interleaved, Word32Swap, ROM_CONTINUE, M reload, BadCrc, Dir vs Zip, find_def)
- Clippy clean, fmt OK

### Fait — T006 ng-testkit comparateur d'images golden
- crates/ng-testkit/src/golden.rs : load_png_rgb, diff_frame, write_diff_ppm
- Dépendance ajoutée : png (décodage PNG)
- 5 tests unitaires passent (roundtrip PNG, diff identique, pixel modifié, taille erronée, écriture PPM)
- Gate OK

### Fait — T005 Dépendances CPU : puce-m68k et puce-z80 (git épinglé) et test de fumée
- cargo fetch + build -p ng-core OK (dépendances git épinglées dans Cargo.toml)
- Test de fumée crates/ng-core/tests/cpu_crates.rs écrit avec 3 tests :
  - test_m68k_reset_move_stop : reset à 40 ticks, MOVE.W à 48 ticks, STOP puis step()=4 ticks
  - test_m68k_wait_states : MOVE.W sans attente = 16 ticks, avec wait_clocks(2) = 18 ticks
  - test_z80_step_api : LD A,1 ; ADD A,A ; HALT → A=2 et 15 T-states
- Gate OK (cargo fmt --check, clippy clean, tests passent sur tout le workspace)

### Fait — T004 Configuration locale (neogeo.example.toml)
- neogeo.example.toml créé avec chemins BIOS/ROMs/saves et config système (mvs, sp-s2.sp1, euro)
- .gitignore contient déjà neogeo.toml (vérifié par git check-ignore)
- TOML valide (testé avec tomllib Python)
- Gate OK : 26 tests passent

### Fait — T001 ng-common horloge, framebuffer, entrées, CRC
- clock.rs : constantes de timing Neo Geo (MCLK_PER_*, PIXELS_PER_LINE, etc.)
- video.rs : Framebuffer 320x224 RGBA8 avec crc32() et write_ppm()
- input.rs : PadState, SystemButtons, trait InputProvider (bits actifs-BAS)
- crc.rs : CRC-32/ISO-HDLC sans dépendance externe (table OnceLock)
- lib.rs : ré-exports publics de toutes les interfaces gelées
- 19 tests unitaires passent (timing, framebuffer, CRC, PPM, entrées)
- Clippy clean pour ng-common ; fmt OK

### Piège rencontré — T001
- write_file bloque si le fichier a été lu avec pagination ; utiliser patch ou re-lire sans offset/limit.
- La table CRC doit être nommée `table()` (snake_case) sinon clippy warning.

### Fait — T000 Workspace Cargo vérifié, outillage de gate, métriques
- tools/check_metrics.ps1 écrit et testé (détection de régression, création de baseline)
- Tests triviaux ajoutés dans les 7 crates lib.rs (ng-gui exclu, main.rs)
- .gitignore déjà complet (neogeo.toml inclus)
- Gate OK : cargo check + clippy + tests passent sur tout le workspace

### Piège rencontré — T000
- PowerShell interprète `$name` comme une référence de lecteur ; utiliser `${name}` dans les chaînes interpolées.

## Prochaine carte
T000 (workspace) -> T001 -> T004 -> T005 (dépendances CPU) -> T006, puis phases suivantes (voir tasks/INDEX.md). Profil Hermes : `rust-dev`.

## Décisions
- 2026-11-08 : RESET v5. CPU = crates de l'utilisateur `puce-m68k` et `puce-z80` (git épinglé, voir Cargo.toml et ref/cpu-crates/README.md). Anciennes cartes/crates archivés dans `_to_delete/reset_20261108/`. 65 cartes, toutes pour le profil `rust-dev`. Rien n'a encore été compilé sur le PC de l'utilisateur.
- 2026-11-02 : projet initialisé avec méthode v2 (cartes, oracles, gate, budget de contexte).

## Pièges connus
(vide)

## Blocages
(vide)

## Cartes régénérées (v4, audit complet du 2026-11-02)
65 cartes (v5, reset du 2026-11-08) dans tasks/ (INDEX.md = ordre des dépendances ; board.json ; scripts/kanban_import.ps1). Les générations précédentes sont dans _to_delete/old_tasks et _to_delete/old_tasks_v2.
- Audit automatique : 0 alerte (dépendances acycliques et existantes, plages de lignes valides, lectures source <= ~1100 lignes, aucune formule vague).
- Les tables générées (crates/ng-lspc/src/{dac,zoom}_table.rs, crates/ng-cart/src/{types,defs,cmc_tables}.rs) COMPILENT (cargo check --workspace vérifié côté cloud).
- Oracles GÉNÉRÉS (versionnés) : tests-data/ym (ymfm réel, 3 voies, T505), tests-data/prot (CRC des fonctions MAME réelles, T700/T701/T703a/T704/T705a), tests-data/scenes (rendu LSPC de référence Python, T406). Outils et conventions : tools/README-golden.md ; régénération : `powershell tools/oracles.ps1` (g++ + python). Pas de #[ignore] : leur absence fait échouer les tests. T712 supprimée. Limites : le brouillage M1 (cmc50_m1_decrypt) n'est vérifié que par le CRC MAME ; les scènes sprites sont synthétiques.
- Référence vidéo : MAME 0.287 sur X:\Mame_0287 ; `powershell tools/mame_golden.ps1 -Game mslug -Frames 120,600,1500 -Out tests-data/golden/mslug`.
- Nom de dépôt/exécution : le gate (scripts/gate.ps1) tourne sur le PC de l'utilisateur ; Claude ne compile que pour vérifier les tables générées.

### Fait — T600a ng-core Machine : structure, construction, reset (Claude)
- `machine.rs` : `Machine` (cpu, z80, bus, sound, lspc/pal/io/backup en `Rc<RefCell>`, latch, rtc, cart, bios, wd, fb, audio). Régions P/M/V et BIOS main/sm1 déplacées (mem::take) vers bus/carte son/ROM ADPCM. `reset()` : 68000 jusqu'à la 1re frontière d'instruction, now == 80, vecteurs lus dans le BIOS.
- `shared.rs` : adaptateurs `Shared<T: Device>`, `SharedLspc`, `SharedPalette` (écritures octet fusionnées).
- `ym_adapter.rs` réécrit (la version agent bouclait sur Default) : séparation `YmIntf` (interface hôte partagée avec la puce : temps, timers, IRQ, busy, ROM) / `YmAdapter` (puce + échantillons, 432 mclk/échantillon). Écart assumé par rapport à la carte (un seul type impossible car la puce garde un Rc vers son interface).
- `Host68k` est minimal (pas de catch-up) : à étendre en T600b.

### Fait — T600b ng-core ordonnanceur à l'horloge (Claude)
- `host68k.rs` : `Host68k` (read/write avec catch-up, ipl = avance LSPC puis `irq_level`, int_ack None, on_tick now+=2), `Machine::split()`, `catch_up` (LSPC rendu, Z80 au T-state, YM, RTC, watchdog), `after_write` (commande son, verrou $3A, chien de garde). `sched.rs` : `run_frame`, `finish_instruction`. `wait_clocks` = 0 jusqu'à T601.
- PIÈGE corrigé dans ng-bus/map68k.rs : le bus alignait l'adresse, donc `Io` (décodage par parité d'adresse) ne voyait jamais d'octet impair : commande son et kick watchdog ($300001) étaient perdus. Conversion uds/lds -> adresse/donnée ajoutée pour les régions E/S. Les écritures MOT vers E/S restent décodées comme avant (octet bas).
- Carte : « écriture 0x3A0003 (q1=1) » est inexacte : le latch utilise A4 comme valeur, donc $3A0013 = q1=1 et $3A0003 = q1=0.
- Le reset fait maintenant un catch-up (Z80 à ~78 mclk après reset).

### Fait - T600c ng-core démarrage et API publique (Claude)
- `boot::load_machine(cfg, game)` : zip dans roms_dir, find_def, load_cart, load_bios, Machine::new, rechargement de `<game>.sram` ; `save_machine` l'écrit.
- `Machine` : set_inputs, framebuffer, take_audio, frame_count (frame_start / MCLK_PER_FRAME), save_backup/load_backup (64 Kio).
- Une trame = ~938 échantillons (55555,56 Hz sur 405 504 mclk à 59,18 Hz ; le 925 du cahier supposait 60 Hz).
- Le Z80 démarre au reset (`sound.reset()` dans `Machine::reset`). Protection de cartouche : stub, rien à installer.

### Fait - T603 Metal Slug 1 : démarrage et attract mode (Claude)
- `crates/ng-core/tests/mslug1.rs` (#[ignore], ROM locale) : 600 trames sans double faute, SP en RAM, fb(120) != fb(600), diff <= 2 % avec les goldens MAME des trames 120 et 600.
- Piège : le BIOS attend les broches du uPD4990A (TP, DATA OUT en $320001) et la réponse du Z80 ($320000) ; `Host68k` les recopie dans `Io` et relaie REG_RTCCTRL ($380051) vers le RTC.
- `tools/mame_snap.lua` : la souscription au notifier doit rester dans une globale (sinon ramasse-miettes Lua, aucune capture).
- Goldens, cfg/ et nvram/ de MAME restent locaux (.gitignore). Suite : T604 savestates.

### Fait - T604 savestates (Claude)
- `crates/ng-core/src/state.rs` : `Machine::save_state(&mut self) -> Vec<u8>` et `load_state(&[u8]) -> Result<(), String>` ; format NGST, version 1, blocs bincode 1.3 ; CRC32 de la ROM P vérifié (Err contenant « crc » pour un autre jeu).
- Feature `serde` dans ng-ym2610, ng-lspc, ng-bus (activée par ng-core) et puce-z80. Tableaux > 32 éléments : module `serde_arr` local à chaque crate (ng-common est gelé).
- YM2610 : les `Box<dyn YmfmInterface>` sont ignorés à la sérialisation ; `Ym2610::restore` remet les interfaces hôtes après désérialisation.
- `save_state` termine l'instruction du 68000, fait le catch-up, puis amène le Z80 à une frontière d'instruction.
- Tests : `state::` (4) et `mslug_savestate_resumes_identically` (ROM locale : 300 trames, save, 120 trames ; reprise identique image + état).
- Limites : état STOP/arrêt du 68000 (Q-M68KSTATE) et état d'un port de protection de cartouche non sauvegardés (à compléter en T702/T703b/T705b).

### Fait - T707 / T711 / T708 / T709 / T710 protections et validation des Metal Slug X, 3, 4, 5 (Claude, reprise du worker)
- T707 mslugx : `crates/ng-core/tests/mslugx.rs` (dump PPM, port installé, compteur d'accès $2FFFE0-$2FFFEF > 0, goldens 120/600/1500). `CartPort` gagne `access_count()` et `bank_change_count()` (défaut 0).
- T711 : `Cart::decrypt_all()` (`ng-cart/src/install.rs`, selon la table `Protection`), `boot::make_port` (dans ng-core, pas ng-bus : ng-bus ne dépend pas de ng-cart) et `load_machine` qui déchiffre puis installe le port.
- PIÈGE T710 (mslug5) : la sélection de banque standard ($2FFFF0-$2FFFFE) interceptait les écritures avant le port PVC ; le jeu bouclait en reboot. Le port a maintenant la priorité (`map68k.rs`). PVC : le changement de banque se déclenche dès l'offset >= 0xFF8 (MAME write_bankprot_pvc), pas > 0xFF8.
- PIÈGE T708 (mslug3) : `mslug3_bank_base` utilisait le mauvais bitswap ; MAME : `bitswap<6>(sel, 9, 3, 6, 15, 12, 14)`.
- PIÈGE T709 (mslug4) : la carte exige une réponse du Z80 en < 100 trames ; le BIOS n'envoie sa première commande son qu'à ~ trame 240-280 (MAME pareil, mêmes instants à 0,03 trame près). Fenêtre portée à 600 trames.
- Autres corrections : lecture C-ROM hors plage = transparent (`draw_pixel`, la ROM C de mslug3/4/5 fait 48-64 Mio) ; `gfx_decrypt` accepte 0x3000000/0x6000000 (cas spécial MAME) ; identification des fichiers d'un zip par CRC (`green.neo-sma`) ; taille S 0x80000 pour MS3/MS4 (MAME).
- MAME et goldens : démarrage à froid avec nvram/cfg vierges (une backup RAM valide fait sauter le test de 64 Ko du BIOS). PIÈGE MAJEUR : pour mslug3 la sortie de MAME dépend de l'exécution (une capture à la trame 120 change tout le déroulé : démo directe vs écran-titre de ~300 trames). `tools/mame_golden.ps1` lance donc une exécution MAME par capture. Notre émulateur suit le déroulé « non perturbé » : commandes son et reboot (chien de garde) aux mêmes instants que MAME à 0,03 trame près. La scène de démo à la trame 1500 diffère encore de MAME : le test mslug3_1500 vérifie seulement une image de jeu plausible (>= 64 couleurs, pas de couleur dominante) ; écart de bits bas du compteur d'animation auto ($3C0006) entre MAME et nous à étudier.
- Suite : compteur d'animation automatique LSPC (bits 0-2 de $3C0006), puis T712+.

### Fait - T800a ng-gui : application eframe (Claude)
- `emu_thread.rs` (thread d'émulation, `Cmd`/`FrameMsg`, `spawn` renvoie `EmuHandle {tx, rx, err_rx, join}` : écart assumé pour pouvoir joindre le thread), `app.rs` (texture NEAREST, échelle entière), `main.rs` (`neogeo.toml` + jeu en argument). eframe 0.37 : le trait `eframe::App` utilise `ui(&mut self, ui, frame)` et non `update`.
- Pacing : cible `t0 + trames x 405504/24e6 s` ; si > 200 ms de retard on repart de maintenant. `#![allow(dead_code)]` dans main.rs jusqu'à T802.

### Fait - T800b ng-gui : menus, liste des jeux, capture (Claude)
- `settings.rs` (chemin config = argument 1 ou `./neogeo.toml`, formulaire 3 dossiers via rfd, `config_to_toml`), `menu.rs` (`available_games`, `shot_name`, `MenuAction`, `Scale`), `app.rs` refondu (l'émulateur démarre seulement si la config est lue). egui 0.37 : `egui::MenuBar::new().ui`, `ui.close()`, `CentralPanel::show(ui, ..)`.
- Écart : le jeu n'est plus un argument de la ligne de commande (l'argument 1 est la config) ; capture PPM écrite par l'application depuis la dernière trame (pas de `fb.write_ppm`).

### Fait - T801a ng-gui : sortie audio cpal (Claude)
- `audio.rs` : `AudioOut` (cpal 0.18 : `build_output_stream(config.into(), ..)` par valeur), `fill_buffer`, `push_capped` (plafond rate/2 valeurs). Le flux est créé DANS le thread d'émulation (cpal::Stream n'est pas Send). Pas de périphérique : message « audio désactivé » sur `err_rx`, l'émulation continue.

## T801b — Entrées clavier + manettes (ng-gui)
- `input.rs` : `InputState` (clavier egui + gilrs), table de touches par défaut (P1 flèches/Z/X/A/S, P2 IJKL/G/H/T/Y, Entrée=Start1, P=Start2, 5/6=Coin, 9=Service, F2=Test, Maj=Select1), stick analogique seuil 0.5, bits actifs-bas.
- `App` envoie `Cmd::Inputs` à chaque frame. 5 tests unitaires.

## T802a — Débogueur registres + mémoire
- `ng-core::debug` : `DebugSnapshot`, `Machine::debug_snapshot()` (termine l'instruction), `peek68k()` sans effet de bord (vecteurs/BIOS/P/RAM, 0xFF ailleurs). `Cmd::Debug(bool)` + 3e canal `dbg_rx` (bounded 2). `debugger.rs` : fenêtre registres 68000/Z80 + RAM 0x100000 (show_rows), `hex_line`. Menu « Débogueur ».
- Écart : la visionneuse lit la copie `work_ram` du snapshot (pas `peek68k`, réservé aux outils).

## T802b — Débogueur vidéo
- `DebugSnapshot` étendu : `vram` (0x8800), `palette` (0x2000), `lspc: LspcDebug`, `fix_image` (320x256). `video_debug.rs` : palette 16x16 par page (32 pages, tooltip index+mot), table des sprites actifs (SCB1-4, chaînage), image fix, registres LSPC. 4 tests.
- Écarts : (1) l'image fix est rendue côté ng-core (la ROM S/BIOS y est accessible) puis affichée par le GUI ; (2) ajout de `Vram::get_offset()` dans ng-lspc et de la dépendance ng-lspc dans ng-gui (pour `Palette`).

## T900 — Banc criterion
- `crates/ng-core/benches/frame.rs` (300 trames mslug) ; `Machine::stats` (`catch_ups`, `z80_ticks`) ; `docs/PERF.md` : 5.91 ms/trame, ~35 % du budget (~2,9x temps réel).
- Suite : T900b (catch-up paresseux) doit conserver les goldens identiques.

## T900b - Catch-up paresseux
- Voir docs/PERF.md. Gain mesuré -22,1 % (5.91 -> 4.60 ms/trame). Tests : `sched::lazy_catch_up_gives_identical_state` (synthétique) et `tests/lazy_catch_up.rs` (mslug, #[ignore]).
- Piège : avec un simple saut de catch-up, l'état du Z80 divergeait (niveau IRQ du YM échantillonné à granularité différente). Corrigé en rendant le YM dépendant du temps du Z80, pas de la fréquence des rattrapages ; goldens inchangés.
- Écart à la carte : le cache `next_event_global` n'est pas utilisé (le Z80 est dû presque à chaque accès bus) ; le saut se décide par zone d'adresse.
- Test adapté : `machine::reset_restores_a_clean_state` n'exige plus que le Z80 ait déjà tourné après le reset (les lectures de vecteurs BIOS ne rattrapent plus).

## T901 - Précision restante
- (1) Bus ouvert : mécanisme `Machine::open_bus_follows_prefetch` (dernier mot lu par le 68000), test `sched::open_bus_returns_last_word_only_when_enabled`. **Désactivé par défaut** : MAME renvoie $FFFF (non confirmé par la comparaison MAME ; la carte n'autorise que les points confirmés). Les goldens sont donc inchangés.
- (2) Ordre à égalité de tick (LSPC -> YM -> Z80 -> 68k) : non modifié, aucune différence MAME observée ; depuis T900b le YM est avancé à l'instant du Z80.
- (3) Wait states : en attente d'une ROM de test matérielle (T902), aucune table modifiée.

## T902 - ROMs de test matérielles
- `crates/ng-core/tests/hwtests.rs` : parcourt `tests-data/hwtests/<nom>/hw.toml` (game, frames, max_diff_percent), exécute N trames, compare à `golden/f<N>.png` (320x224 exigé), écrit `target/hw-report.md` et les diffs PPM. Dossier vide ou fichiers manquants : ignoré sans échec. 5 tests ; vérifié à la main avec un cas mslug (0 pixel de différence).
- `docs/spec/data/test-roms.md` : gabarit, procédure, écarts MAME/matériel. `tools/mame_golden.ps1` : exemple d'usage hwtests dans l'en-tête.
- Écart : aucune ROM de test matérielle n'est encore déposée (liste vide) ; format `hw.toml` simple (pas de crate toml).

## T903 - Memory card
- `ng-bus::memcard::MemCard` (2 Kio, octets impairs, `present`/`protected`, load/save) remplace `MemCardStub` ; `Io::card_present` pilote les bits 12-14 de $380000 (0x7000 absente, 0 présente) ; `Machine::memcard` partagée avec le bus ; `load_memcard`/`save_memcard`.
- Persistance : `saves_dir/<profil>.memcard` (profil = nom du BIOS de neogeo.toml, caractères non alphanumériques -> `_`), 2048 octets. La carte est présente si ce fichier existe au `load_machine` ; `insert_memcard(cfg)` crée une carte vierge ; `save_machine` la réécrit.
- Écarts : pas de verrous `REG_CRDLOCK` ni du bit de protection en lecture de $380000 (seulement `protected` côté carte) ; la carte n'est pas dans les savestates ; pas de menu GUI pour insérer la carte ; test de boot AES non ajouté (BIOS AES non disponible ici).

## T804a - Préférences persistantes
- `ng-gui/src/prefs.rs` : `Prefs { display, input, debug }` dans `gui.toml` (même dossier que `neogeo.toml`), `to_toml`/`from_toml` (lignes invalides ignorées), `load` (absent = défauts), `save` atomique (`write_atomic` : `.tmp` + renommage). `App` sauvegarde quand les préférences changent et à la fermeture. L'échelle et l'ouverture du débogueur sont déjà restaurées.
- Ajout par rapport à la carte : `DebugPrefs::region` (région de l'onglet Mémoire, T805b).

## T804b - Menu BIOS et modèle
- `bios_menu.rs` : `KNOWN_BIOS` (BIOS MVS 128 Kio de MAME, Universe BIOS 4.0, BIOS AES), `available_bios` (présence dans `neogeo.zip` ou le dossier, ROM communes exigées), `updated_toml` (ne remplace que `bios =` et `model =`, garde le reste du fichier). Menu Système > BIOS / Modèle ; `Cmd::Config` ; le jeu courant est relancé. Message d'erreur si le BIOS configuré est absent.
- Écarts : présence testée par lecture directe (une ouverture de l'archive au lieu d'un `load_bios` par BIOS) ; noms Universe BIOS/AES ajoutés hors fichier de référence ; le menu BIOS n'est pas ouvert automatiquement (message seulement).

## T804c - Configuration des touches
- `input.rs` : `Action` (24 actions, `TABLE` = nom, libellé, cible, défauts), `Bindings` (clavier + manette, `set_key`/`set_button` avec échange, `reset`, `from_prefs`/`to_prefs`), `InputState::with_bindings`. `controls_menu.rs` : fenêtre Contrôles (Émulation > Contrôles…), capture par clic, Échap annule, défauts par joueur ou globaux. Sauvegarde dans `gui.toml` ([input.keys], [input.pad]).
- Détails : `none` = action sans touche ; Maj reste Select J1 tant qu'aucune touche ne lui est assignée ; l'échange de boutons ne concerne que la même manette ; pendant une capture les entrées ne sont pas envoyées au jeu. Échap ne peut pas être assignée.

## T805a - Affichage pixel perfect et plein écran
- `display.rs` : `layout()` calcule en pixels physiques (facteur entier >= 1, coin sur un pixel entier, HiDPI pris en compte) ; image peinte directement (`painter.image`) sur fond noir, sans marge, NEAREST (LINEAR si Lissage). Menu Vidéo : Échelle (Auto, x1-x8), Plein écran, Ratio, Lissage ; tout est dans `gui.toml`.
- Plein écran : F11 (bascule), Échap (sortie), restauré au démarrage ; barre de menus masquée, révélée quand la souris touche le bord haut ; curseur masqué après 2 s ; barre d'état masquée.
- Écart : le ratio 4:3 est 320x240 par facteur (960x720 à x3), pas « 960x672 » comme écrit par erreur dans la carte ; il étire les lignes, donc n'est pas pixel perfect.

## T805b - Débogueur à onglets, barre d'état
- `ng-core::debug` : `DebugLevel { Off, Cpu, Memory, Video, Full }`, `debug_snapshot_level` (ne copie que le nécessaire) ; `DebugSnapshot` gagne `frame`, `m68k_halted` ; `LspcDebug` gagne `anim_speed`, `anim_disabled`, `irq_level`, `pal_bank`, `shadow`, `fix_from_cart`.
- `emu_thread` : `Cmd::Debug(DebugLevel)`, `Cmd::StepFrame` ; instantané une trame sur deux en marche, immédiat en pause quand l'onglet change ou après un pas.
- `debugger.rs` : une fenêtre, onglets CPU / Mémoire / Vidéo / Palette / Sprites, boutons Pause et Trame suivante. CPU : PC (adresse de l'instruction), IR, SR + drapeaux, D0-D7/A0-A7, USP/SSP ; Z80 complet + drapeaux ; niveau d'IRQ. Mémoire : RAM 68000, RAM Z80, RAM palette, VRAM ; « Aller à » ; colonne ASCII. Valeurs modifiées surlignées.
- `statusbar.rs` : `format_status` (jeu · BIOS · modèle · trame · fps · échelle · MARCHE/PAUSE). Menu Débogage ; raccourcis F5 pause, F6 trame suivante, F9 capture, F12 débogueur. Onglet, région et état de la fenêtre dans `gui.toml`.
- Écart : `DebugLevel::Memory` copie les quatre régions (150 Ko) plutôt que la seule région choisie. Dépendance `puce-m68k` ajoutée à ng-gui (type `CpuState`).

## T805c - Débogueur vidéo
- `video_debug.rs` réécrit en trois onglets. Vidéo : registres LSPC en clair (ligne, IRQ, adresse/pas VRAM, timer décodé, auto-animation, banque de palette, source du fix), couche fix x2 avec grille 8x8, cadre de la zone visible, survol d'une tuile (adresse VRAM, code, palette). Palette : banque (active signalée), page de 16 palettes, survol/clic (copie `#RRGGBB`), couleur de fond. Sprites : liste des sprites actifs avec X/Y/hauteur effectifs (chaînes suivies comme MAME), filtre par ligne, tri, compteurs.
- Fonctions pures testées : `decode_scb2/3/4`, `sprite_list`, `on_line`, `sprite_row`, `sort_sprites`, `tile_at`, `fix_tile_addr/label`, `pen_label`, `irq2_text`.
- Écart : tri par liste déroulante (pas par clic sur l'en-tête) ; seule la première tuile de chaque sprite est décodée.

## Retouches après essai réel du GUI (captures de la fenêtre)
- Le débogueur est un panneau ancré à droite (plus une fenêtre flottante qui cachait le jeu) ; surlignage des changements seulement en pause ; lignes mémoire sans retour à la ligne ; fix x2 seulement si le panneau est assez large.
- `ng-gui <config> <jeu>` : le 2e argument lance un jeu au démarrage. F1 ouvre Contrôles. Noms de touches lisibles (« Flèche haut »).

## T1001 - Catalogue : romsets MAME et défs générées (128 jeux sans protection)
- `tools/catalogue.py` (liste PLAIN, PROTECTED, TO_EXTRACT, TITLES) partagé par `tools/extract_romsets.py` (-> `docs/spec/data/romsets_all.txt`, 158 `ROM_START`, aucune donnée de ROM) et `tools/gen_tables.py` (défs Rust). `ALL_CARTS` = 134 définitions ; les 6 Metal Slug sont identiques à avant (vérifié par diff, hors champ `vb`).
- Nouveautés générateur : `NEO_BIOS_AUDIO_64K/256K`, `NEO_SFIX_64K`, `ROM_RELOAD` (irrmaze), région ADPCM-B `Region::VB` + `RegionSizes.vb` + `Cart.vb`, régions inconnues ignorées (mainbios, mcu...), regex insensibles à la casse. Identifiants Rust des jeux commençant par un chiffre : préfixe `N_` (`N_2020BB`).
- Écart : liste des jeux dans `tools/catalogue.py` (et non dans gen_tables.py) pour la partager avec l'extracteur.
- GUI : le menu Fichier > Ouvrir ne liste plus que les jeux dont le zip est présent, triés par titre, dans une zone défilante (`installed_games`).
- Suite : T1002 (ADPCM-B dans le YM), T1003 (CRC réels), T1004 (banc).

## T1002 - ADPCM-B sur sa propre ROM
- `YmIntf.adpcm_b_rom` ; `YmAdapter::new(a, b)` ; `external_read(AdpcmB)` lit B si non vide, sinon A (comportement MAME). `Machine::new` déplace `cart.vb`. Tests : `adpcm_b_reads_its_own_rom_with_fallback_to_a`, `regions_are_moved_to_their_owners`.

## T1003 - Intégrité du catalogue
- `crates/ng-cart/tests/catalogue.rs` (#[ignore], `NG_ROMS`, `NG_GAMES`; jeu inconnu => compté ABSENT). Bilan réel : OK 134 / ABSENT 0 / ECHEC 0.
- Piège : `puzzldpr` est un split set (zip sans c1/c2/m1/v1) -> `load_cart` cherche désormais par CRC dans les zips voisins (repli parent, `read_from_sibling_zip`).

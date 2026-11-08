# 02 — Vidéo LSPC : sprites, fix, palette (sources : wiki Sprites, Sticky_bit, Sprite_shrinking, Sprite_graphics_format, Fix_layer, Fix_graphics_format, Palettes, Palette_RAM, Colors, Video_DAC, Auto_animation, Line_buffers, Sprites_per_line_limit, Frame_size)
Complète 00-clock-timing.md (timing, VRAM) et 01 (registres).

## Écran
Visible : NTSC 320x224 (PAL 320x256). Le compteur de ligne visible va de $110 à $1EF (224 lignes). Colonnes visibles : 320 pixels de l'intervalle actif.
Coordonnées sprite -> écran : Y écran = 496 - Y_SCB3 (mesuré depuis le haut de la bordure, soit "ligne $100"?) ; X écran mesuré depuis la bordure gauche. Les offsets exacts (quel pixel est le 0 visible) ne sont pas donnés par le wiki : A COMPLETER par calibration sur BIOS (voir tâche T40x "calibrage").

## Sprites (448 en VRAM, 381 évalués par ligne, 96 rendus par ligne max)
Tuile 16x16, 4 bpp, 128 octets, couleur 0 transparente. Un sprite = bande verticale d'1 tuile de large, hauteur 1..32 tuiles. Numéro de tuile sur 20 bits.
### SCB1 ($0000-$6FFF) : carte de tuiles ; 64 mots / sprite (index s : base = s*64), 2 mots par tuile, de haut en bas
- Mot pair (2*t) : bits 15-0 = n° de tuile bits 15-0.
- Mot impair (2*t+1) : bits 15-8 = palette (256 palettes) ; bits 7-4 = n° de tuile bits 19-16 (le wiki ne marque explicitement que le bit 4 ; la tuile est sur 20 bits donc bits 7..4) ; bit 3 = auto-anim 8 tuiles ; bit 2 = auto-anim 4 tuiles ; bit 1 = flip vertical ; bit 0 = flip horizontal.
  (Le découpage 15-8 palette / 7-4 MSB tuile est l'organisation standard MAME ; le wiki est incomplet -> A VÉRIFIER avec le BIOS.)
### SCB2 ($8000-$81FF) : un mot par sprite (index s)
- bits 7-0 : shrink vertical ($FF = taille réelle).
- bits 11-8 : shrink horizontal ($F = taille réelle).
- autres bits libres (relisibles).
### SCB3 ($8200-$83FF) : un mot par sprite
- bits 15-7 : position Y (9 bits). Position écran = 496 - Y.
- bit 6 : sticky bit (le wiki dit "bit 6" dans le texte mais "bit 1" dans le tableau ; convention MAME/hardware = bit 6 -> utiliser bit 6, A VÉRIFIER).
- bits 5-0 : hauteur en tuiles (0..33 ; 33 = 32 tuiles avec repli des bordures lors du shrink ; >=33 se comporte comme 32 avec wrap).
### SCB4 ($8400-$85FF) : un mot par sprite
- bits 15-7 : position X (9 bits). bits 6-0 inutilisés.
Wrap sur 512 pixels en X et en Y.
### Chaînage (sticky bit = 1)
Le sprite se place immédiatement à droite du précédent (X précédent + 16 en non zoomé ; avec shrink H du précédent la largeur avance de sa largeur réduite : A VÉRIFIER). Hérite : Y, hauteur, shrink vertical du pilote. Garde : son propre shrink horizontal. Ignore tous ses autres attributs SCB3/SCB4. Les valeurs VRAM ne sont pas modifiées.
### Évaluation par ligne
Pour chaque ligne visible N : parcours des sprites 1..381 (le #0 est réservé) ; un sprite est actif sur la ligne si (ligne - Y) tombe dans sa fenêtre (hauteur x 16 lignes avec shrink vertical via table L0 ROM, voir ci-dessous). Max 96 retenus, dans l'ordre d'index ; les suivants sont perdus (coût 16 px/sprite = 1536 px par ligne, transparents ou rétrécis inclus).
### Ordre de dessin
Les sprites retenus sont dessinés dans l'ordre de la liste active (index croissant) ; un pixel écrit remplace le précédent ("overwrite") donc l'index le plus élevé est devant. (Le wiki : "Overlapping sprites just overwrite the previously written pixels".) Un pixel de couleur 0 ne se dessine pas (transparent) : A VÉRIFIER (le wiki ne le dit pas explicitement pour le buffer, mais le fix/sprite utilisent la couleur 0 comme transparente).
Priorité finale : fond (backdrop) < sprites < fix (le fix est toujours au-dessus).

### Shrink horizontal (table du wiki = MAME ; 1 = pixel dessiné ; colonne = pixel 0..15 de la ligne de tuile ; v = nibble SCB2 bits 11-8)
```
0: 0000000010000000
1: 0000100010000000
2: 0000100010001000
3: 0010100010001000
4: 0010100010001010
5: 0010101010001010
6: 0010101010101010
7: 1010101010101010
8: 1010101011101010
9: 1011101011101010
A: 1011101011101011
B: 1011101111101011
C: 1011101111101111
D: 1111101111101111
E: 1111101111111111
F: 1111111111111111
```
Le shrink horizontal ne dépend d'aucune ROM. Le coût de rendu reste de 16 mclk par ligne de tuile (pixels sautés inclus). La valeur ne se propage PAS aux sprites chaînés (chacun a la sienne).

### Shrink vertical
Valeur 8 bits (SCB2 bits 7-0), $FF = taille réelle, $00 = 1/256. Les lignes à afficher sont déterminées par une table dans la ROM L0 (non reproduite sur le wiki). Le fichier est `000-lo.lo` du jeu de BIOS neogeo (fourni par l'utilisateur, 64 Kio dans MAME). Format MAME (à confirmer par la carte T4xx) : pour chaque valeur de shrink (0..255), 256 octets indexés par le numéro de ligne dans la fenêtre du sprite ; l'octet donne (n° de ligne de tuile 0..15 dans les bits bas, n° de tuile dans les bits hauts) ; la fenêtre est la hauteur définie (non rétrécie), les lignes au-delà de 16 tuiles répètent la dernière ligne de la tuile d'index 15. Le sous-échantillonnage est sans lissage.

## Fix layer
Non défilante, au-dessus des sprites. Tuile 8x8, 4 bpp, 32 octets, couleur 0 transparente, palettes 0-15 seulement. Carte en VRAM $7000-$74FF : 40x32 mots, une entrée = 1 mot. Ordre de la carte : colonne par colonne de haut en bas, de gauche à droite (adresse = $7000 + colonne*32 + ligne ; REG_VRAMMOD = 32 sert aux mises à jour par ligne). Entrée : bits 15-12 = palette (0-15), bits 11-0 = n° de tuile (4096 max sans bankswitch). Zone dessinée NTSC : 40 colonnes x 28 lignes (2 lignes cachées en haut et en bas, soit lignes de carte 2..29 -> A VÉRIFIER). Source de tuiles : S ROM de la cartouche ou SFIX (embarquée dans le BIOS) selon REG_CRTFIX/REG_BRDFIX. Avec CMC : tuiles >4096 via extension VRAM $7500+ (voir Fix_bankswitching, carte protections).
### Format d'une tuile fix (32 octets)
Octet = 2 pixels, pixel gauche = bits 0-3, pixel droit = bits 4-7. Adresse dans la tuile = H*16 + C*8 + L (L = ligne 0-7, C = colonne dans la moitié, H = moitié : 0 = droite, 1 = gauche) -> les colonnes sont stockées de haut en bas par paires de pixels. ATTENTION : ce déchiffrage est une inférence du wiki, la figure n'est pas lisible dans la source. La carte de rendu fix contient un test de calibrage : le texte du BIOS (écran self-test) doit être lisible ; en cas d'échec, tester la variante MAME (ligne = 4 octets, pixels 0-3 aux octets 2-3 avec nibble bas = premier pixel, pixels 4-7 aux octets 0-1).

## Format des tuiles sprite (C ROM)
Tuile 16x16 4 bpp = 128 octets = quatre blocs 8x8 dans l'ordre : haut-droite, bas-droite, haut-gauche, bas-gauche. Chaque ligne de bloc est stockée à l'envers (pixel 7 d'abord), en plans de bits : un octet par plan. C ROM impaires (C1, C3...) : plans 0 et 1 ; C ROM paires (C2, C4...) : plans 2 et 3. Dans chaque ROM une tuile = 64 octets : 4 blocs x 8 lignes x 2 octets (plan bas, plan haut alternés). (MAME : chargement par paires ROM_LOAD16_BYTE ; ordre exact octet/plan des tableaux "Odd C ROMs/Even C ROMs" non reproduit par le wiki lu -> la carte T404 le fixe par test sur une tuile synthétique et le BIOS.)

## Auto-animation
Un seul compteur interne de 3 bits (lisible dans les 3 bits bas de REG_LSPCMODE). Vitesse = bits 15-8 de REG_LSPCMODE : le compteur 3 bits s'incrémente tous les (n+1) frames ; la nouvelle vitesse ne s'applique qu'au prochain rechargement du compteur interne 8 bits (décrémenté 1 fois par trame, rechargé à l'underflow avec la dernière valeur écrite). Mode par tuile (SCB1 mot impair) : bits 3,2 = 00 aucune ; 01 anim 4 tuiles (les 2 bits bas du n° de tuile sont REMPLACÉS par les 2 bits bas du compteur) ; 10 ou 11 anim 8 tuiles (3 bits bas remplacés). REG_LSPCMODE bit 3 = 1 : auto-animation coupée pour toutes les tuiles (le compteur tourne quand même).

## Palette
RAM 16 Kio = 2 banques x 256 palettes x 16 couleurs x 16 bits, une seule banque visible (REG_PALBANK0/1). Accès 68k à $400000 (octet : valeur copiée sur les 2 octets du mot). Les sprites utilisent les 256 palettes, le fix les 16 premières. Couleur 0 de chaque palette = transparente. $400000 (couleur 0 palette 0) = couleur de référence, doit être $8000 (noir). Couleur de fond (backdrop) = dernière couleur de la banque, $401FFE (couleur 15?? du palette 255 -> mot à l'index 255*16+0 ? : le wiki dit "dernière couleur de la banque" à $401FFE donc palette 255 couleur 15 ; MAME utilise palette 255 couleur 0 = $401FE0...). CONFLIT -> la carte vidéo utilise l'adresse $401FFE du wiki et un test BIOS (fond du self-test) pour trancher.
Mot couleur : bit 15 = "dark" (bit commun), bits 14-12 = R0,G0,B0, bits 11-8 = R4..R1, bits 7-4 = G4..G1, bits 3-0 = B4..B1. Donc v5(R) = ((w>>8)&0xF)<<1 | ((w>>14)&1), v5(G) = ((w>>4)&0xF)<<1 | ((w>>13)&1), v5(B) = (w&0xF)<<1 | ((w>>12)&1).
Conversion vers RGB8 : TABLE EXACTE issue de MAME (réseau de résistances 3900/2200/1000/470/220 ohms ; pulldown 8200 ohms pour le bit dark ; 150 ohms pour SHADOW) : `docs/spec/data/dac_lookup.txt`, 32 lignes `index5 normal dark shadow dark+shadow` (valeurs 0..255), générée par `tools/gen_dac.py`. Canal8 = table[v5][colonne] avec colonne = dark + 2*shadow (0 normal, 1 dark, 2 shadow, 3 dark+shadow). `shadow` = REG_SHADOW (latch Q0). Exemple : v5=31 -> 255 (normal), 251 (dark), 142 (shadow), 141 (dark+shadow).
Banc de palette : 2 x 4096 mots ; le banc visible est choisi par Q7 du latch système. Couleur de fond (backdrop) = mot d'index $FFF (4095) du banc visible = adresse $401FFE (palette 255, couleur 15). (Confirmé par MAME : `m_bg_pen = pen_base + 0xfff`.) Le fond remplit l'écran avant les sprites ; les pixels de sprite de couleur 0 ne sont pas dessinés.

## Line buffers
2 paires (A/B), chacune pair/impair, entrées de 12 bits (8 bits palette + 4 bits couleur), une paire écrite pendant que l'autre sort. Les pixels de sprites s'écrivent un par un à leur X ; un pixel écrit remplace l'ancien. Le fix n'est pas dans le buffer (superposé en sortie). Dans MAME : le fond est rempli d'abord (pen $FFF), puis les sprites de la liste, puis le fix par-dessus ; un pixel de valeur 0 n'écrit pas. Modèle matériel (wiki) : buffers effacés à $FFF.

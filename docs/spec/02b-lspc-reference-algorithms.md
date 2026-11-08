# 02b — Algorithmes de référence du LSPC (traduction fidèle de MAME ref/mame-neogeo/neogeo_spr.cpp, neogeo_v.cpp)
Ce sont les règles que le code Rust doit reproduire à l'identique (sortie pixel-perfect de référence). Les noms sont ceux de MAME.

## Accès VRAM (neogeo_spr.cpp l.76-110)
```
vram: [u16; 0x8000 + 0x800]   // 0x0000-0x7FFF lente ; 0x8000-0x87FF rapide (index = adresse & 0x87FF)
set_offset(data): offset = if data & 0x8000 != 0 { data & 0x87FF } else { data };  read_buffer = vram[offset]   // la lecture est immédiate
get_data() -> read_buffer
set_data(d): vram[offset] = d; set_offset((offset & 0x8000) | ((offset + modulo) & 0x7FFF))   // l'incrément ne touche pas le bit 15
modulo : i16 (registre 16 bits), lecture/écriture libre
```
Registre mot $3C0000/$3C0002/$3C0004 : lecture $3C0000 et $3C0002 renvoient read_buffer ; $3C0004 renvoie modulo ; $3C0006 renvoie get_video_control(). Accès octet : seule l'écriture sur l'octet haut (UDS) est valide, la valeur est dupliquée sur les 2 octets ; l'accès octet bas seul n'est pas mappé (lecture = bus ouvert, écriture ignorée).

## get_video_control (neogeo_v.cpp l.176)
`v = vpos + 0x100; if v >= 0x200 { v -= 264 };  return (v << 7) | (auto_anim_counter & 7)` où vpos = ligne MAME 0..263 (vpos 0 = début de trame ; lignes visibles vpos 16..239).

## Écriture $3C0006 (set_video_control)
`auto_anim_speed = data >> 8 ; auto_anim_disabled = bit 3 ; irq2_control = data & 0x00F0` (bit4 enable, bit5 load-relative, bit6 autoload-vblank, bit7 autoload-repeat).

## Auto-animation (l.136)
**Corrigé (MAME neogeo_spr.cpp l.150-166)** : une fois par trame à vpos 256 (VSSTART = 0x100, soit 256*1536 mclk après le début de trame), PAS à la ligne 240 : `if frame_counter == 0 { frame_counter = speed; counter += 1 } else { frame_counter -= 1 }` (counter u8, seuls 3 bits bas utilisés par le rendu).

## Parsing des sprites pour une ligne `scanline` (parse_sprites, l.462)
```
list = if scanline & 1 { vram[0x8680..] } else { vram[0x8600..] }
y = 0; rows = 0; count = 0
for sprite in 0..381 {                       // MAX_SPRITES_PER_SCREEN = 381 ; le sprite 0 est parcouru aussi
    y_control = vram[0x8200 | sprite]
    if bit6(y_control) == 0 { y = 0x200 - (y_control >> 7); rows = y_control & 0x3F }   // non chaîné : nouvelles valeurs
    if rows == 0 { continue }
    if !sprite_on_scanline(scanline, y, rows) { continue }
    list[count] = sprite; count += 1
    if count == 96 { break }
}
remplir list[count ..= 96] avec 0     // une entrée de plus que nécessaire
sprite_on_scanline(scanline, y, rows) = rows == 0 || rows >= 0x20 || ((scanline - y) & 0x1FF) < rows * 16
```
(Note : `rows >= 0x20` => sprite toujours "sur la ligne" : hauteur 32 tuiles = 512 lignes = pleine boucle verticale.)
Quand parser ? MAME : au DÉBUT de la ligne N (vpos = N), parse(N) avec l'état VRAM de cet instant ; dessin de la ligne N-1 au même instant avec l'état VRAM de cet instant. Hors v-visible les lignes sont aussi parsées (compteur complet 0..263).

## Dessin des sprites sur une ligne (draw_sprites, l.283)
```
list = (scanline & 1) ? vram[0x8680..] : vram[0x8600..]
max_idx = dernière entrée non nulle de list[0..96) ; si max_idx != 95 { max_idx += 1 }  // optimisation MAME ; matériel : parcourt toutes les 96, le sprite 0 en fin de liste est inoffensif car rows==0 ou hors écran — pour l'exactitude, parcourir list[0..=max_idx]
for k in 0..=max_idx {
    sprite = list[k] & 0x1FF
    y_control = vram[0x8200 | sprite]; zoom_control = vram[0x8000 | sprite]
    if bit6(y_control) {                      // chaîné
        x = (x + zoom_x + 1) & 0x1FF; zoom_x = (zoom_control >> 8) & 0xF
    } else {
        y = 0x200 - (y_control >> 7); x = vram[0x8400 | sprite] >> 7
        zoom_y = zoom_control & 0xFF; zoom_x = (zoom_control >> 8) & 0xF; rows = y_control & 0x3F
    }
    if 0x140 <= x && x <= 0x1F0 { continue }  // entièrement hors écran
    if !sprite_on_scanline(scanline, y, rows) { continue }
    sprite_line = (scanline - y) & 0x1FF
    zoom_line = sprite_line & 0xFF; invert = bit8(sprite_line)
    if invert { zoom_line ^= 0xFF }
    if rows > 0x20 {
        zoom_line %= (zoom_y + 1) << 1
        if zoom_line > zoom_y { zoom_line = ((zoom_y + 1) << 1) - 1 - zoom_line; invert = !invert }
    }
    b = zoomy_rom[(zoom_y << 8) | zoom_line]           // 000-lo.lo
    sprite_y = b & 0xF; tile = b >> 4
    if invert { sprite_y ^= 0xF; tile ^= 0x1F }
    attr = vram[(sprite << 6) | (tile << 1) | 1]; code = ((attr << 12) & 0xF0000) | vram[(sprite << 6) | (tile << 1)]
    if !auto_anim_disabled { if bit3(attr) { code = (code & !7) | (counter & 7) } else if bit2(attr) { code = (code & !3) | (counter & 3) } }
    if bit1(attr) { sprite_y ^= 0xF }                  // flip vertical
    pal = attr >> 8                                     // palette 0..255
    hflip = bit0(attr)
    xtable = ZOOM_X[zoom_x]                             // u16 : bit15 = pixel 0 à tester d'abord
    // ligne de 16 pixels de la tuile (code, sprite_y) :
    //   pixels en ordre gauche->droite si !hflip, droite->gauche si hflip
    //   pour i in 0..16 : if xtable bit15 { émettre pixel i ; x_dest += 1 } ; xtable <<= 1 ; if xtable == 0 { break }
    //   émission = si pixel_color != 0 alors ligne[x_dest] = palette[pal][pixel_color]
    //   x_dest = x (0..0x1F0) ; si x > 0x1F0 : wrap-around, les pixels dont x_dest >= 0x200 sont écrits à partir de la colonne 0
}
ZOOM_X = [0x0080,0x0880,0x0888,0x2888,0x288A,0x2A8A,0x2AAA,0xAAAA,0xAAEA,0xBAEA,0xBAEB,0xBBEB,0xBBEF,0xFBEF,0xFBFF,0xFFFF]   // = table du wiki, vérifiée sur matériel
```
Décodage d'un pixel (C ROM, région "sprites" = C1/C2 entrelacés par octet, voir 04) pour le pixel x (0..15 de la ligne de tuile, déjà éventuellement inversé par hflip) à la ligne `sprite_y` (0..15) de la tuile `code` :
```
romaddr = (code << 8) | (sprite_y << 4) | x            // x = 0..15, hflip : on part de x=15 et on décrémente
base = ((romaddr & !0xFF) >> 1) | (((romaddr & 8) ^ 8) << 3) | ((romaddr & 0xF0) >> 2)     // = code*128 + (x<8 ? 64 : 0) + sprite_y*4
bit = romaddr & 7
color = (bit(region[base+3], bit) << 3) | (bit(region[base+1], bit) << 2) | (bit(region[base+2], bit) << 1) | bit(region[base+0], bit)
```
Adresse masquée par la taille de région (puissance de 2 supérieure). 

## Dessin du fix sur une ligne (draw_fixed_layer, l.180)
```
src = if fixed_layer_source { S ROM cartouche } else { sfix.sfix du BIOS }
for x in 0..40 {
    w = vram[0x7000 | (scanline >> 3)  + x * 0x20]    // colonne x, ligne de tuile scanline>>3
    code = w & 0x0FFF; pal = w >> 12
    (banking CMC : voir fix_bankswitching, types GAROU/KOF2000 — Metal Slug 3 utilise le type GAROU : table 'garouoffsets' l.189-209 et code += 0x1000 * (garouoffsets[(y-2)&31] ^ 3))
    off = ((code << 5) | (scanline & 7)) & addr_mask
    pixels 0,1 <- src[off + 0x10] ; pixels 2,3 <- src[off + 0x18] ; pixels 4,5 <- src[off + 0x00] ; pixels 6,7 <- src[off + 0x08]
    chaque octet : pixel gauche = bits 0-3, pixel droit = bits 4-7 ; si valeur != 0 alors écrire palette[pal][valeur]
}
```
(`scanline` ici = ligne visible 0..223 dans le bitmap ; le fix dessiné ligne par ligne par-dessus les sprites.) Colonnes visibles : le bitmap MAME fait 320 pixels visibles = 40 tuiles.

## Ordre de rendu d'une image (screen_update)
1. Remplir la ligne avec le fond (pen d'index $FFF du banc visible, avec shadow si REG_SHADOW). 2. draw_sprites. 3. draw_fixed_layer.
La palette utilisée est celle de l'instant du dessin de la ligne (les écritures palette en milieu d'image sont visibles ligne par ligne).

## Interruptions et timer (neogeo.cpp l.752-875)
```
VBLANK_IRQ : à chaque trame, à la ligne 240 (vpos), 56+2 = 58 mclk après le front montant de HSYNC : vblank_pending = 1 -> IRQ niveau 1
display_position_vblank : à la ligne 240, 1146 mclk après HSYNC : si irq2_control.bit6 (autoload vblank) { recharger le timer }
timer : compteur 32 bits `display_counter` (valeur de reload) ; on arme un événement dans (display_counter + 1) pixels (= *4 mclk)
        quand l'événement tombe : if irq2_control.bit4 { raster_pending = 1 (IRQ niveau 2) } ; if bit7 { réarmer (+ display_counter+1 pixels) }
écriture TIMERHIGH ($3C0008) : display_counter = (display_counter & 0xFFFF) | (data << 16)
écriture TIMERLOW ($3C000A) : display_counter = (display_counter & 0xFFFF0000) | data ; if bit5 { armer le timer }
IRQACK ($3C000C) : bit0 -> irq3_pending = 0 ; bit1 -> raster_pending = 0 ; bit2 -> vblank_pending = 0
niveaux : IRQ1 = vblank, IRQ2 = raster, IRQ3 = irq3 (posé au premier démarrage)
```
Coordonnées MAME : HTOTAL = 0x180 = 384 pixels, HBEND = 0x1C (28), HBSTART = 0x15C (348) => pixels visibles x = 28..347 (320), VTOTAL = 0x108 = 264, VBEND = 16, VBSTART = 240 => lignes visibles 16..239 (224).

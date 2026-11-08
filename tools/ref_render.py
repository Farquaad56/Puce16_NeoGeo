#!/usr/bin/env python3
"""Rendu de référence LSPC (oracle Python, indépendant du Rust), porté de MAME neogeo_spr.cpp (BSD-3).
Usage : python tools/ref_render.py <dossier_scène>   -> écrit expected.raw (224*320 u32 LE, 0x00RRGGBB)
Dossier de scène : vram.bin (0x8800 u16 LE) pens.bin (>=0x1000 u32 LE : tranche de base de la palette) c.bin s.bin lo.bin meta.txt
meta.txt : lignes "clé valeur" : anim_counter, anim_disabled (0/1), fix_bank (0 Std,1 Garou,2 Kof2000), from_cart (0/1)
Séquence (= sprite_line_timer_callback) : pour line in 0..264 : si line != 0 : render(line-1) ; parse(line).
"""
import struct, sys, os
ZOOM_X=[0x0080,0x0880,0x0888,0x2888,0x288a,0x2a8a,0x2aaa,0xaaaa,0xaaea,0xbaea,0xbaeb,0xbbeb,0xbbef,0xfbef,0xfbff,0xffff]
def load(d):
    rd=lambda n:open(os.path.join(d,n),'rb').read()
    vram=list(struct.unpack('<%dH'%0x8800,rd('vram.bin')))
    pb=rd('pens.bin'); pens=list(struct.unpack('<%dI'%(len(pb)//4),pb))
    meta=dict(l.split() for l in open(os.path.join(d,'meta.txt')) if l.strip())
    return vram,pens,rd('c.bin'),rd('s.bin'),rd('lo.bin'),{k:int(v) for k,v in meta.items()}
def on_line(sl,y,rows): return rows==0 or rows>=0x20 or ((sl-y)&0x1ff)<rows*0x10
def parse(vram,sl):
    base=0x8680 if sl&1 else 0x8600
    y=0; rows=0; cnt=0; p=base
    for n in range(381):
        yc=vram[0x8200|n]
        if not (yc&0x40): y=0x200-(yc>>7); rows=yc&0x3f
        if rows==0: continue
        if not on_line(sl,y,rows): continue
        vram[p]=n; p+=1; cnt+=1
        if cnt==96: break
    for k in range(96-cnt+1): vram[p+k]=0
def gmask(n):
    m=0xFFFFFFFF
    for b in range(31,-1,-1):
        if ((n*2-1)>>b)&1: break
        m>>=1
    return m
def draw_pixel(c,ra,out,x,lp):
    s=((ra&~0xff)>>1)|(((ra&8)^8)<<3)|((ra&0xf0)>>2); xb=ra&7
    g=(((c[s+3]>>xb)&1)<<3)|(((c[s+1]>>xb)&1)<<2)|(((c[s+2]>>xb)&1)<<1)|((c[s]>>xb)&1)
    if g and 0<=x<320: out[x]=lp[g]
def draw_sprites(vram,c,lo,pens,anim,animdis,sl,out,mask):
    lst=0x8680 if sl&1 else 0x8600
    mx=95
    while mx>=0 and vram[lst+mx]==0: mx-=1
    if mx!=95: mx+=1
    y=x=rows=zy=zx=0
    for si in range(mx+1):
        n=vram[lst+si]&0x1ff; yc=vram[0x8200|n]; zc=vram[0x8000|n]
        if yc&0x40:
            x=(x+zx+1)&0x1ff; zx=(zc>>8)&0xf
        else:
            y=0x200-(yc>>7); x=vram[0x8400|n]>>7; zy=zc&0xff; zx=(zc>>8)&0xf; rows=yc&0x3f
        if 0x140<=x<=0x1f0: continue
        if on_line(sl,y,rows):
            sline=(sl-y)&0x1ff; zl=sline&0xff; inv=bool(sline&0x100)
            if inv: zl^=0xff
            if rows>0x20:
                zl%=((zy+1)<<1)
                if zl>zy: zl=((zy+1)<<1)-1-zl; inv=not inv
            t=lo[(zy<<8)|zl]; sy=t&0xf; tile=t>>4
            if inv: sy^=0xf; tile^=0x1f
            ao=(n<<6)|(tile<<1); attr=vram[ao+1]; code=((attr<<12)&0xf0000)|vram[ao]
            if not animdis:
                if attr&8: code=(code&~7)|(anim&7)
                elif attr&4: code=(code&~3)|(anim&3)
            if attr&2: sy^=0xf
            zt=ZOOM_X[zx]; gb=((code<<8)|(sy<<4))&mask
            lp=pens[(attr>>8)<<4:]; inc=1
            if attr&1: gb+=0xf; inc=-1
            if x<=0x1f0:
                px=x
                for i in range(16):
                    if zt&0x8000: draw_pixel(c,gb,out,px,lp); px+=1
                    zt=(zt<<1)&0xffff
                    if zt==0: break
                    gb+=inc
            else:
                xs=x
                for i in range(16):
                    if zt&0x8000:
                        if x>=0x200: draw_pixel(c,gb,out,x-0x200,lp)
                        x+=1
                    zt=(zt<<1)&0xffff
                    if zt==0: break
                    gb+=inc
                x=xs
def draw_fix(vram,gfx,from_cart,bank,pens,sl,out):
    am=len(gfx)-1; banked=from_cart and am>0x1ffff
    vd=0x7000|(sl>>3); go=[0]*34
    if banked and bank==1:
        gbk=0;k=0;y=0
        while y<32:
            if vram[0x7500+k]==0x0200 and (vram[0x7580+k]&0xff00)==0xff00:
                gbk=vram[0x7580+k]&3; go[y]=gbk; y+=1
            go[y]=gbk; y+=1; k+=2
    px=0
    for x in range(40):
        cp=vram[vd]; code=cp&0xfff
        if banked:
            y=sl>>3
            if bank==1: code+=0x1000*(go[(y-2)&31]^3)
            elif bank==2: code+=0x1000*(((vram[0x7500+((y-1)&31)+32*(x//6)]>>((5-(x%6))*2))&3)^3)
        off=((code<<5)|(sl&7))&am; cpens=pens[(cp>>12)<<4:]
        for o in (0x10,0x18,0x00,0x08):
            d=gfx[off+o]
            if d&0x0f: out[px]=cpens[d&0x0f]
            px+=1
            if d&0xf0: out[px]=cpens[d>>4]
            px+=1
        vd+=0x20
def render(d):
    vram,pens,c,s,lo,m=load(d); mask=gmask(len(c)); fb=[]
    rows={}
    for line in range(264):
        if line!=0:
            v=line-1
            if 16<=v<=239:
                out=[pens[0xfff]]*320
                draw_sprites(vram,c,lo,pens,m['anim_counter'],m['anim_disabled'],v,out,mask)
                draw_fix(vram,s,m['from_cart'],m['fix_bank'],pens,v,out)
                rows[v-16]=out
        parse(vram,line)
    flat=[p for r in range(224) for p in rows[r]]
    open(os.path.join(d,'expected.raw'),'wb').write(struct.pack('<%dI'%len(flat),*flat))
if __name__=='__main__': render(sys.argv[1])

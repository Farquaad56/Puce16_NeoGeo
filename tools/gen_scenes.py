#!/usr/bin/env python3
"""Génère les scènes synthétiques déterministes (graine fixe) dans tests-data/scenes/ puis appelle ref_render.render."""
import random, struct, os, sys
sys.path.insert(0, os.path.dirname(__file__))
import ref_render
def rb(r,n): return bytes(r.getrandbits(8) for _ in range(n))
def make(name,seed,fix_only=False,sprites=0,from_cart=0,s_size=0x20000,fix_bank=0,chain=False,anim=(0,0),zoom=True):
    r=random.Random(seed); d=os.path.join('tests-data','scenes',name); os.makedirs(d,exist_ok=True)
    vram=[0]*0x8800
    for col in range(40):
        for row in range(32): vram[0x7000+col*32+row]=(r.randrange(16)<<12)|r.randrange(0x1000)
    if fix_bank==1:
        vram[0x7500]=0x0200; vram[0x7580]=0xFF02; vram[0x7502]=0x0200; vram[0x7582]=0xFF01
    if fix_bank==2:
        for i in range(0x80): vram[0x7500+i]=r.getrandbits(16)
    c=rb(r,0x80000)
    for n in range(sprites):
        for k in range(64): vram[(n<<6)+k]=r.getrandbits(16) if k&1==0 else (r.getrandbits(16)&0xff0f|(r.randrange(16)<<4)&0)  # mots impairs : attr
        zx=r.randrange(16) if zoom else 15; zy=r.randrange(256) if zoom else 255
        vram[0x8000+n]=(zx<<8)|zy
        ch= chain and n>0 and r.random()<0.4
        rows=r.choice([1,2,4,8,16,32,33])
        vram[0x8200+n]=(r.randrange(0x100,0x200)<<7)|(0x40 if ch else 0)|(rows&0x3f) if not ch else (0x40)
        vram[0x8400+n]=r.randrange(0x1f0 if not ch else 1)<<7 if True else 0
        if not ch and n%3==0: vram[0x8200+n]=((0x200-r.randrange(16,200))<<7)|(rows&0x3f)
    pens=[r.getrandbits(24) for _ in range(0x1000)]
    lo=rb(r,0x20000); s=rb(r,s_size)
    open(os.path.join(d,'vram.bin'),'wb').write(struct.pack('<%dH'%0x8800,*vram))
    open(os.path.join(d,'pens.bin'),'wb').write(struct.pack('<%dI'%0x1000,*pens))
    open(os.path.join(d,'c.bin'),'wb').write(c); open(os.path.join(d,'s.bin'),'wb').write(s); open(os.path.join(d,'lo.bin'),'wb').write(lo)
    open(os.path.join(d,'meta.txt'),'w').write('anim_counter %d\nanim_disabled %d\nfix_bank %d\nfrom_cart %d\n'%(anim[0],anim[1],fix_bank,from_cart))
    ref_render.render(d); print(name,'ok')
make('fix_std',1)
make('sprites_plain',2,sprites=40,zoom=False)
make('sprites_zoom_chain',3,sprites=60,chain=True,anim=(5,0))
make('fix_garou',4,sprites=20,from_cart=1,s_size=0x40000,fix_bank=1)
make('fix_kof2000',5,from_cart=1,s_size=0x40000,fix_bank=2)

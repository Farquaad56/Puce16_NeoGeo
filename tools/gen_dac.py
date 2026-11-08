# Reproduit create_rgb_lookups() de MAME (neogeo_v.cpp) + compute_resistor_weights() (resnet.cpp)
R=[3900,2200,1000,470,220]
def weights(scaler,pd):
    w=[]
    for n in range(5):
        R0=1.0/1e12 if pd==0 else 1.0/pd
        R1=1.0/1e12
        for j in range(5):
            if j==n: R1+=1.0/R[j]
            else: R0+=1.0/R[j]
        R0=1.0/R0; R1=1.0/R1
        v=255.0*R0/(R1+R0)
        w.append(min(max(v,0.0),255.0))
    mx=sum(w)
    sc = 255.0/mx if scaler<0 else scaler
    return [x*sc for x in w], sc, mx
wn,sc,_=weights(-1,0)
wd,_,_=weights(sc,8200)
ws,_,_=weights(sc,150)
wds,_,_=weights(sc,1.0/((1.0/8200)+(1.0/150)))
rows=[]
for i in range(32):
    bits=[(i>>k)&1 for k in range(5)]
    f=lambda w:int(sum(w[k]*bits[k] for k in range(5))+0.5)
    rows.append((f(wn),f(wd),f(ws),f(wds)))
with open("dac_lookup.txt","w") as fo:
    fo.write("# index5 : normal dark shadow dark+shadow  (valeurs 0..255) — généré par tools/gen_dac.py d'après MAME neogeo_v.cpp\n")
    for i,r in enumerate(rows): fo.write(f"{i} {r[0]} {r[1]} {r[2]} {r[3]}\n")
print(rows[0],rows[1],rows[16],rows[31]); print("scaler",sc)

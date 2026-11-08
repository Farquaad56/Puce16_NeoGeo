// Oracle YM2610 : pilote ymfm (BSD-3). Génère tests-data/ym/*.bin (i16 LE, 3 voies par échantillon [FM gauche, FM droite, SSG mono], 1000 échantillons)
// Conventions (identiques à l'adaptateur Rust T600a) :
//  - 1 échantillon toutes les 144 horloges YM (432 mclk) ; l'échantillon n (n>=1) est produit à ym_time = n*144.
//  - Avant de produire l'échantillon n : on exécute les écritures planifiées "avant l'échantillon n" (ym_time = (n-1)*144),
//    puis on traite les timers dont l'échéance <= n*144 (ordre chronologique), puis generate().
//  - set_timer(t, d) : échéance = ym_time + d (d<0 => timer arrêté).
//  - ROM ADPCM : rom(a) = h ^ (h>>15) & 0xFF avec h = a * 0x9E3779B1 (u32).
#include "ymfm_opn.h"
#include <cstdio>
#include <vector>
#include <string>
#include <algorithm>
using namespace ymfm;
static uint8_t rom(uint32_t a){ uint32_t h=a*0x9E3779B1u; h^=h>>15; return (uint8_t)(h&0xFF); }
struct Intf : ymfm_interface {
  uint64_t t=0; int64_t dl[2]={-1,-1}; uint64_t busy=0; bool irq=false;
  void ymfm_set_timer(uint32_t n,int32_t d) override { dl[n]= d<0 ? -1 : (int64_t)(t+d); }
  void ymfm_set_busy_end(uint32_t c) override { busy=t+c; }
  bool ymfm_is_busy() override { return t<busy; }
  void ymfm_update_irq(bool a) override { irq=a; }
  uint8_t ymfm_external_read(access_class, uint32_t a) override { return rom(a); }
  void fire(uint32_t n){ m_engine->engine_timer_expired(n); }
};
struct W { int before; int off; int val; };   // off 0..3
static void run(const std::string&name,std::vector<W> w,bool status=false){
  Intf in; ym2610 chip(in,0x36); chip.set_fidelity(OPN_FIDELITY_MED); chip.reset();
  std::vector<int16_t> out; std::vector<uint8_t> st;
  size_t wi=0;
  for(int n=1;n<=1000;n++){
    in.t=(uint64_t)(n-1)*144;
    while(wi<w.size()&&w[wi].before<=n){ chip.write(w[wi].off,w[wi].val); wi++; }
    uint64_t target=(uint64_t)n*144;
    for(;;){ int b=-1; for(int k=0;k<2;k++) if(in.dl[k]>=0&&(uint64_t)in.dl[k]<=target&&(b<0||in.dl[k]<in.dl[b])) b=k;
      if(b<0)break; in.t=in.dl[b]; in.dl[b]=-1; in.fire(b); }
    in.t=target;
    ym2610::output_data o; chip.generate(&o,1);
    for(int c=0;c<3;c++){ int v=o.data[c]; v=std::max(-32768,std::min(32767,v)); out.push_back((int16_t)v); }
    if(status) st.push_back(chip.read(0));
  }
  FILE*f=fopen(("out/ym/"+name+".bin").c_str(),"wb"); fwrite(out.data(),2,out.size(),f); fclose(f);
  if(status){ f=fopen(("out/ym/"+name+"_status.bin").c_str(),"wb"); fwrite(st.data(),1,st.size(),f); fclose(f); }
  long nz=0; for(auto v:out) nz+= v!=0; printf("%-10s non-zero=%ld\n",name.c_str(),nz);
}
// aides : lo(r,v) = registre r banque basse ; hi(r,v) = banque haute (r+0x100)
static void lo(std::vector<W>&w,int b,int r,int v){ w.push_back({b,0,r}); w.push_back({b,1,v}); }
static void hi(std::vector<W>&w,int b,int r,int v){ w.push_back({b,2,r}); w.push_back({b,3,v}); }
int main(){
  run("silence",{});
  { std::vector<W> w; // FM canal 1 (offset 1), algo 7
    lo(w,1,0xB1,0x07); lo(w,1,0xB5,0xC0);
    for(int op=0;op<4;op++){ int o=op*4+1; lo(w,1,0x30+o,0x01); lo(w,1,0x40+o,0x10); lo(w,1,0x50+o,0x1F); lo(w,1,0x60+o,0x05); lo(w,1,0x70+o,0x02); lo(w,1,0x80+o,0x24); }
    lo(w,1,0xA5,0x22); lo(w,1,0xA1,0x69); lo(w,2,0x28,0xF1); run("fm",w); }
  { std::vector<W> w; lo(w,1,0,0x10); lo(w,1,1,0); lo(w,1,7,0x3E); lo(w,1,8,0x0F); run("ssg",w); }
  { std::vector<W> w; hi(w,1,0x01,0x3F); hi(w,1,0x08,0xDF); hi(w,1,0x10,0); hi(w,1,0x18,0); hi(w,1,0x20,0x02); hi(w,1,0x28,0); hi(w,2,0x00,0x01); run("adpcma",w); }
  { std::vector<W> w; lo(w,1,0x1B,0xFF); lo(w,1,0x11,0xC0); lo(w,1,0x12,0); lo(w,1,0x13,0); lo(w,1,0x14,1); lo(w,1,0x15,0); lo(w,1,0x19,0); lo(w,1,0x1A,0x40); lo(w,1,0x10,0x01); lo(w,2,0x10,0x80); run("adpcmb",w); }
  { std::vector<W> w; lo(w,1,0x24,0xFC); lo(w,1,0x25,0x00); lo(w,1,0x26,0xF0); lo(w,1,0x27,0x3F); lo(w,400,0x27,0x15); run("timers",w,true); }
  { std::vector<W> w; lo(w,1,0xB1,0x07); lo(w,1,0xB5,0xC0);
    for(int op=0;op<4;op++){ int o=op*4+1; lo(w,1,0x30+o,0x01); lo(w,1,0x40+o,0x20); lo(w,1,0x50+o,0x1F); lo(w,1,0x60+o,0x05); lo(w,1,0x70+o,0x02); lo(w,1,0x80+o,0x24); }
    lo(w,1,0xA5,0x22); lo(w,1,0xA1,0x69); lo(w,2,0x28,0xF1);
    lo(w,1,0,0x20); lo(w,1,1,0); lo(w,1,7,0x3E); lo(w,1,8,0x0A);
    hi(w,1,0x01,0x3F); hi(w,1,0x08,0xDF); hi(w,1,0x10,0); hi(w,1,0x18,0); hi(w,1,0x20,0x02); hi(w,1,0x28,0); hi(w,3,0x00,0x01); run("mix",w); }
}

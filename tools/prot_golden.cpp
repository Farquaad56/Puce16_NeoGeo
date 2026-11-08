// Oracle protections : applique les fonctions MAME (prot_*.cpp, BSD-3) à des tampons déterministes et écrit le CRC32 du résultat.
// Entrée : pat(i) = (uint8_t)((i * 0x9E3779B1u) >> 24)  (i = index d'octet, u32 avec retenue modulo 2^32).
// Pour les P ROM (MS3, MS5) : le tampon Rust `p` est big-endian 68000 ; MAME travaille en mémoire hôte (x86) où chaque mot est
// stocké little-endian => le harnais échange les octets de chaque paire avant, et après, l'appel MAME. Le CRC porte sur l'ordre Rust.
#include "emu.h"
#include "prot_cmc.h"
#include "prot_sma.h"
#include "prot_pvc.h"
#include "prot_pcm2.h"
static uint8_t pat(uint32_t i){ return (uint8_t)((i*0x9E3779B1u)>>24); }
static uint32_t crc32(const uint8_t*p,size_t n){ uint32_t c=0xFFFFFFFF; for(size_t i=0;i<n;i++){ c^=p[i]; for(int k=0;k<8;k++) c=(c>>1)^(0xEDB88320u&-(c&1)); } return ~c; }
static std::vector<uint8_t> mk(size_t n){ std::vector<uint8_t> v(n); for(size_t i=0;i<n;i++) v[i]=pat((uint32_t)i); return v; }
static void swp(std::vector<uint8_t>&v){ for(size_t i=0;i+1<v.size();i+=2) std::swap(v[i],v[i+1]); }
static void out(const char*name,const std::vector<uint8_t>&v){ char f[256]; snprintf(f,sizeof f,"out/prot/%s.crc",name); FILE*o=fopen(f,"w"); fprintf(o,"%08X\n",crc32(v.data(),v.size())); fclose(o); printf("%-24s %08X\n",name,crc32(v.data(),v.size())); }
int main(){
  machine_config mc; cmc_prot_device cmc(mc,"c",nullptr,0); sma_prot_device sma(mc,"s",nullptr,0); pvc_prot_device pvc(mc,"p",nullptr,0); pcm2_prot_device pcm(mc,"q",nullptr,0);
  { auto r=mk(0x10000); cmc.cmc42_gfx_decrypt(r.data(),r.size(),0xAD); out("cmc42_010000_kAD",r); }
  { auto r=mk(0x10000); cmc.cmc42_gfx_decrypt(r.data(),r.size(),0x00); out("cmc42_010000_k00",r); }
  { auto r=mk(0x10000); cmc.cmc50_gfx_decrypt(r.data(),r.size(),0x31); out("cmc50_010000_k31",r); }
  { auto r=mk(0x10000); cmc.cmc50_gfx_decrypt(r.data(),r.size(),0x19); out("cmc50_010000_k19",r); }
  { auto r=mk(0x10000); std::vector<uint8_t> fx(0x1000); cmc.sfix_decrypt(r.data(),r.size(),fx.data(),fx.size()); out("sfix_010000_1000",fx); }
  { auto cr=mk(0x80000); std::vector<uint8_t> au(0x90000); cmc.cmc50_m1_decrypt(cr.data(),cr.size(),au.data(),au.size()); out("m1_crypt_080000",cr); out("m1_audio_090000",au); }
  { auto p=mk(0x900000); swp(p); sma.mslug3_decrypt_68k(p.data()); swp(p); out("mslug3_p_900000",p); }
  { auto p=mk(0x800000); swp(p); pvc.mslug5_decrypt_68k(p.data(),p.size()); swp(p); out("mslug5_p_800000",p); }
  { auto v=mk(0x10000); pcm.decrypt(v.data(),v.size(),8); out("pcm2_decrypt8_010000",v); }
  { auto v=mk(0x1000000); pcm.swap(v.data(),v.size(),2); out("pcm2_swap2_1000000",v); }
}

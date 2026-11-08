#pragma once
#include <cstdint>
#include <cstddef>
#include <cstring>
#include <cstdio>
#include <vector>
#include <string>
typedef uint32_t offs_t;
struct machine_config {};
#define ATTR_COLD
#define NAME(x) x
#define DECLARE_DEVICE_TYPE(a,b) extern int a;
#define DEFINE_DEVICE_TYPE(a,b,c,d) int a = 0;
#define BYTE_XOR_LE(a) (a)
struct device_t {
  device_t(const machine_config&, int, const char*, device_t*, uint32_t) {}
  virtual ~device_t() {}
  virtual void device_start() {}
  virtual void device_reset() {}
  template<class T> void save_item(T&&) {}
  struct Sys { const char* name="x"; }; struct Mach { Sys s; Sys& system(){return s;} }; Mach m_; Mach& machine(){return m_;}
};
template<unsigned N, typename T, typename... B> uint32_t bitswap(T v, B... bits) {
  unsigned b[] = { (unsigned)bits... }; uint32_t r = 0;
  for (unsigned i = 0; i < N; i++) r |= (uint32_t)(((uint64_t)v >> b[i]) & 1) << (N - 1 - i);
  return r;
}
namespace util { struct sum16_creator { static uint16_t simple(const void*p, size_t n){ uint16_t s=0; auto q=(const uint8_t*)p; for(size_t i=0;i<n;i++) s+=q[i]; return s; } }; }

#define COMBINE_DATA(p) (*(p) = (*(p) & ~mem_mask) | (data & mem_mask))

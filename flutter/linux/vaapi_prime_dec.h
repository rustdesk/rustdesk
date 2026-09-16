#ifndef RUSTDESK_VAAPI_PRIME_DEC_H_
#define RUSTDESK_VAAPI_PRIME_DEC_H_

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef RUSTDESK_EXPORT
#define RUSTDESK_EXPORT __attribute__((visibility("default")))
#endif

#define RUSTDESK_GPU_FRAME_NV12 1
#define RUSTDESK_GPU_FRAME_PRIME 2

typedef struct RustDeskPrimeFrame {
  int kind;
  int n_fds;
  int fds[4];
  uint32_t fourcc;
  uint64_t modifier;
  int width;
  int height;
  int n_planes;
  int pitches[4];
  int offsets[4];
  int obj_indices[4];
} RustDeskPrimeFrame;

RUSTDESK_EXPORT void* RustDeskVaapiPrimeOpen(int hevc);
RUSTDESK_EXPORT void RustDeskVaapiPrimeClose(void* ctx);
// Returns 1 if `out` is filled with dup'd fds (caller/GL owns them until replaced).
RUSTDESK_EXPORT int RustDeskVaapiPrimeDecode(void* ctx,
                                             const uint8_t* data,
                                             int len,
                                             RustDeskPrimeFrame* out);

#ifdef __cplusplus
}
#endif

#endif

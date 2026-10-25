// OpenHarmony controller build ships without libaom (AV1 is not negotiated and
// the aomedia.googlesource source clone is impractical to fetch), but aom.rs's
// FFI still needs every referenced symbol to exist at link time. These stubs
// return the aom error code so AV1 encoder/decoder construction fails cleanly
// at runtime and the peer falls back to VP8/VP9.
//
// Signatures intentionally use loose pointer types: the caller passes the
// aom_*_t pointers declared in the generated bindings, and the stub only has
// to match the C calling convention and return size.

#include <stddef.h>

#define AOM_CODEC_ERROR 3

void *aom_codec_av1_cx(void) { return 0; }
void *aom_codec_av1_dx(void) { return 0; }

int aom_codec_control(void *ctx, int ctrl_id, ...) { return AOM_CODEC_ERROR; }
int aom_codec_dec_init_ver(void *ctx, void *iface, void *cfg, unsigned long flags, int ver) {
    return AOM_CODEC_ERROR;
}
int aom_codec_decode(void *ctx, const unsigned char *data, unsigned int sz, void *user_priv) {
    return AOM_CODEC_ERROR;
}
int aom_codec_destroy(void *ctx) { return AOM_CODEC_ERROR; }
int aom_codec_enc_config_default(void *iface, void *cfg, int usage) { return AOM_CODEC_ERROR; }
int aom_codec_enc_config_set(void *ctx, const void *cfg) { return AOM_CODEC_ERROR; }
int aom_codec_enc_init_ver(void *ctx, void *iface, const void *cfg, unsigned long flags, int ver) {
    return AOM_CODEC_ERROR;
}
int aom_codec_encode(void *ctx, void *img, long pts, unsigned long dur, int flags,
                     unsigned long deadline) {
    return AOM_CODEC_ERROR;
}
const void *aom_codec_get_cx_data(void *ctx, int *iter) { return 0; }
const void *aom_codec_get_frame(void *ctx, int *iter) { return 0; }
void *aom_img_wrap(void *img, int fmt, unsigned int w, unsigned int h, unsigned int align,
                   unsigned char *data) {
    return 0;
}
void aom_img_free(void *img) {}

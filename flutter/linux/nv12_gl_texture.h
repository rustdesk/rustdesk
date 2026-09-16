#ifndef RUSTDESK_NV12_GL_TEXTURE_H_
#define RUSTDESK_NV12_GL_TEXTURE_H_

#include <flutter_linux/flutter_linux.h>

G_BEGIN_DECLS

void nv12_gl_texture_plugin_register_with_registrar(FlPluginRegistrar* registrar);

G_END_DECLS

#ifdef __cplusplus
extern "C" {
#endif

// Native present path used by librustdesk (RTLD_DEFAULT).
// `texture` is the pointer returned by getTexturePtr.
void RustDeskNv12GlOnNv12(void* texture,
                          const uint8_t* y,
                          int y_stride,
                          const uint8_t* uv,
                          int uv_stride,
                          int width,
                          int height);

#ifdef __cplusplus
}
#endif

#endif

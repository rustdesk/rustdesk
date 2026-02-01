#ifndef FFI_H
#define FFI_H

#include <stdint.h>

void *dxgi_new_capturer(int64_t luid);
void *dxgi_device(void *capturer);
int dxgi_width(const void *capturer);
int dxgi_height(const void *capturer);
void *dxgi_capture(void *capturer, int wait_ms);
void destroy_dxgi_capturer(void *capturer);

#endif // FFI_H
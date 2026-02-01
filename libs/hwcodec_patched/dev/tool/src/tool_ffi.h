#ifndef TOOL_FFI_H
#define TOOL_FFI_H

#include <stdint.h>

void *tool_new(int64_t luid);
void *tool_device(void *tool);
void *tool_get_texture(void *tool, int width, int height);
void tool_get_texture_size(void *tool, void *texture, int *width, int *height);
void tool_destroy(void *tool);

#endif // TOOL_FFI_H
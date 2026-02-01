#ifndef MUX_FFI_H
#define MUX_FFI_H

#include <stdint.h>

void *hwcodec_new_muxer(const char *filename, int width, int height, int is265,
                        int framerate);

int hwcodec_write_video_frame(void *muxer, const uint8_t *data, int len,
                              int64_t pts_ms, int key);
int hwcodec_write_tail(void *muxer);

void hwcodec_free_muxer(void *muxer);

#endif // FFI_H
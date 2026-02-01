#ifndef FFMPEG_VRAM_FFI_H
#define FFMPEG_VRAM_FFI_H

#include "../common/callback.h"
#include <stdbool.h>

void *ffmpeg_vram_new_decoder(void *device, int64_t luid,
                              int32_t codecID);
int ffmpeg_vram_decode(void *decoder, uint8_t *data, int len,
                       DecodeCallback callback, void *obj);
int ffmpeg_vram_destroy_decoder(void *decoder);
int ffmpeg_vram_test_decode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum,
                            int32_t *outDescNum,
                            int32_t dataFormat, uint8_t *data, int32_t length,
                            const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount);
void *ffmpeg_vram_new_encoder(void *handle, int64_t luid,
                              int32_t dataFormat, int32_t width, int32_t height,
                              int32_t kbs, int32_t framerate, int32_t gop);

int ffmpeg_vram_encode(void *encoder, void *tex, EncodeCallback callback,
                       void *obj, int64_t ms);
int ffmpeg_vram_destroy_encoder(void *encoder);

int ffmpeg_vram_test_encode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum,
                            int32_t *outDescNum,
                            int32_t dataFormat, int32_t width, int32_t height,
                            int32_t kbs, int32_t framerate, int32_t gop,
                            const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount);
int ffmpeg_vram_set_bitrate(void *encoder, int32_t kbs);
int ffmpeg_vram_set_framerate(void *encoder, int32_t framerate);

#endif // FFMPEG_VRAM_FFI_H
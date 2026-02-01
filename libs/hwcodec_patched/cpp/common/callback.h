#ifndef CALLBACK_H
#define CALLBACK_H

#include <stdint.h>

typedef void (*EncodeCallback)(const uint8_t *data, int32_t len, int32_t key,
                               const void *obj, int64_t pts);

typedef void (*DecodeCallback)(void *opaque, const void *obj);

#endif // CALLBACK_H
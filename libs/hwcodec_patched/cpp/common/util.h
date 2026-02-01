#ifndef UTIL_H
#define UTIL_H

#include <string>
#include <chrono>
extern "C" {
#include <libavcodec/avcodec.h>
}

namespace util_encode {

void set_av_codec_ctx(AVCodecContext *c, const std::string &name, int kbs,
                      int gop, int fps);
bool set_lantency_free(void *priv_data, const std::string &name);
bool set_quality(void *priv_data, const std::string &name, int quality);
bool set_rate_control(AVCodecContext *c, const std::string &name, int rc,
                      int q);
bool set_gpu(void *priv_data, const std::string &name, int gpu);
bool force_hw(void *priv_data, const std::string &name);
bool set_others(void *priv_data, const std::string &name);

bool change_bit_rate(AVCodecContext *c, const std::string &name, int kbs);
void vram_encode_test_callback(const uint8_t *data, int32_t len, int32_t key, const void *obj, int64_t pts);

} // namespace util

namespace util_decode {
    bool has_flag_could_not_find_ref_with_poc();
}

namespace util {

    inline std::chrono::steady_clock::time_point now() {
        return std::chrono::steady_clock::now();
    }

    inline int64_t elapsed_ms(std::chrono::steady_clock::time_point start) {
        return std::chrono::duration_cast<std::chrono::milliseconds>(now() - start).count();
    }

    inline bool skip_test(const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount, int64_t currentLuid, int32_t dataFormat) {
      for (int32_t i = 0; i < excludeCount; i++) {
        if (excludedLuids[i] == currentLuid && excludeFormats[i] == dataFormat) {
          return true;
        }
      }
      return false;
    }
}


#endif

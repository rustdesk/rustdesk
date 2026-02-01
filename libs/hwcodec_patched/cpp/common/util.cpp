extern "C" {
#include <libavutil/opt.h>
}

#include "util.h"
#include <limits>
#include <map>
#include <string.h>
#include <vector>

#include "common.h"

#include "common.h"

#define LOG_MODULE "UTIL"
#include "log.h"

namespace util_encode {

void set_av_codec_ctx(AVCodecContext *c, const std::string &name, int kbs,
                      int gop, int fps) {
  c->has_b_frames = 0;
  c->max_b_frames = 0;
  if (gop > 0 && gop < std::numeric_limits<int16_t>::max()) {
    c->gop_size = gop;
  } else if (name.find("vaapi") != std::string::npos) {
    c->gop_size = std::numeric_limits<int16_t>::max();
  } else if (name.find("qsv") != std::string::npos) {
    c->gop_size = std::numeric_limits<uint16_t>::max();
  } else {
    c->gop_size = std::numeric_limits<int>::max();
  }
  c->keyint_min = std::numeric_limits<int>::max();
  /* put sample parameters */
  // https://github.com/FFmpeg/FFmpeg/blob/415f012359364a77e8394436f222b74a8641a3ee/libavcodec/encode.c#L581
  if (kbs > 0) {
    int64_t bitrate = kbs * 1000;
    c->bit_rate = bitrate;
    
    // Set rate control parameters to maintain quality at lower FPS
    // Similar to Sunshine's approach: https://github.com/LizardByte/Sunshine/blob/master/src/video.cpp
    c->rc_max_rate = bitrate;
    
    if (name.find("qsv") != std::string::npos) {
      // QSV uses VBR mode with same max rate for better quality
      c->bit_rate--; // cbr with vbr
    } else {
      // For other encoders, set min rate to match max rate for CBR
      c->rc_min_rate = bitrate;
    }
    
    // Set buffer size to one frame worth of data
    // This prevents quality degradation at low FPS by limiting buffering
    if (fps > 0) {
      c->rc_buffer_size = bitrate / fps;
    }
  }
  /* frames per second */
  c->time_base = av_make_q(1, 1000);
  c->framerate = av_make_q(fps, 1);
  c->flags |= AV_CODEC_FLAG2_LOCAL_HEADER;
  c->flags |= AV_CODEC_FLAG_LOW_DELAY;
  c->slices = 1;
  c->thread_type = FF_THREAD_SLICE;
  c->thread_count = c->slices;

  // https://github.com/obsproject/obs-studio/blob/3cc7dc0e7cf8b01081dc23e432115f7efd0c8877/plugins/obs-ffmpeg/obs-ffmpeg-mux.c#L160
  c->color_range = AVCOL_RANGE_MPEG;
  c->colorspace = AVCOL_SPC_SMPTE170M;
  c->color_primaries = AVCOL_PRI_SMPTE170M;
  c->color_trc = AVCOL_TRC_SMPTE170M;

  if (name.find("h264") != std::string::npos) {
    c->profile = FF_PROFILE_H264_HIGH;
  } else if (name.find("hevc") != std::string::npos) {
    c->profile = FF_PROFILE_HEVC_MAIN;
  }
}

bool set_lantency_free(void *priv_data, const std::string &name) {
  int ret;

  if (name.find("nvenc") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "delay", "0", 0)) < 0) {
      LOG_ERROR(std::string("nvenc set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("amf") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "query_timeout", "1000", 0)) < 0) {
      LOG_ERROR(std::string("amf set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("qsv") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "async_depth", "1", 0)) < 0) {
      LOG_ERROR(std::string("qsv set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("vaapi") != std::string::npos) {
    if ((ret = av_opt_set(priv_data, "async_depth", "1", 0)) < 0) {
      LOG_ERROR(std::string("vaapi set_lantency_free failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  if (name.find("videotoolbox") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "realtime", 1, 0)) < 0) {
      LOG_ERROR(std::string("videotoolbox set realtime failed, ret = ") + av_err2str(ret));
      return false;
    }
    if ((ret = av_opt_set_int(priv_data, "prio_speed", 1, 0)) < 0) {
      LOG_ERROR(std::string("videotoolbox set prio_speed failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool set_quality(void *priv_data, const std::string &name, int quality) {
  int ret = -1;

  if (name.find("nvenc") != std::string::npos) {
    switch (quality) {
    // p7 isn't zero lantency
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "preset", "p4", 0)) < 0) {
        LOG_ERROR(std::string("nvenc set opt preset p4 failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "preset", "p1", 0)) < 0) {
        LOG_ERROR(std::string("nvenc set opt preset p1 failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("amf") != std::string::npos) {
    switch (quality) {
    case Quality_High:
      if ((ret = av_opt_set(priv_data, "quality", "quality", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality quality failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "quality", "balanced", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality balanced failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "quality", "speed", 0)) < 0) {
        LOG_ERROR(std::string("amf set opt quality speed failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("qsv") != std::string::npos) {
    switch (quality) {
    case Quality_High:
      if ((ret = av_opt_set(priv_data, "preset", "veryslow", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset veryslow failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    case Quality_Medium:
      if ((ret = av_opt_set(priv_data, "preset", "medium", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset medium failed, ret = ") + av_err2str(ret));
        return false;
      }
      break;
    case Quality_Low:
      if ((ret = av_opt_set(priv_data, "preset", "veryfast", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt preset veryfast failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
      break;
    default:
      break;
    }
  }
  if (name.find("mediacodec") != std::string::npos) {
    if (name.find("h264") != std::string::npos) {
      if ((ret = av_opt_set(priv_data, "level", "5.1", 0)) < 0) {
        LOG_ERROR(std::string("mediacodec set opt level 5.1 failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
    }
    if (name.find("hevc") != std::string::npos) {
      // https:en.wikipedia.org/wiki/High_Efficiency_Video_Coding_tiers_and_levels
      if ((ret = av_opt_set(priv_data, "level", "h5.1", 0)) < 0) {
        LOG_ERROR(std::string("mediacodec set opt level h5.1 failed, ret = ") +
                  av_err2str(ret));
        return false;
      }
    }
  }
  return true;
}

struct CodecOptions {
  std::string codec_name;
  std::string option_name;
  std::map<int, std::string> rc_values;
};

bool set_rate_control(AVCodecContext *c, const std::string &name, int rc,
                      int q) {
  if (name.find("qsv") != std::string::npos) {
    // https://github.com/LizardByte/Sunshine/blob/3e47cd3cc8fd37a7a88be82444ff4f3c0022856b/src/video.cpp#L1635
    c->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
  }
  std::vector<CodecOptions> codecs = {
      {"nvenc", "rc", {{RC_CBR, "cbr"}, {RC_VBR, "vbr"}}},
      {"amf", "rc", {{RC_CBR, "cbr"}, {RC_VBR, "vbr_latency"}}},
      {"mediacodec",
       "bitrate_mode",
       {{RC_CBR, "cbr"}, {RC_VBR, "vbr"}, {RC_CQ, "cq"}}},
      // {"videotoolbox", "constant_bit_rate", {{RC_CBR, "1"}}},
    };

  for (const auto &codec : codecs) {
    if (name.find(codec.codec_name) != std::string::npos) {
      auto it = codec.rc_values.find(rc);
      if (it != codec.rc_values.end()) {
        int ret = av_opt_set(c->priv_data, codec.option_name.c_str(),
                             it->second.c_str(), 0);
        if (ret < 0) {
          LOG_ERROR(codec.codec_name + " set opt " + codec.option_name + " " +
                    it->second + " failed, ret = " + av_err2str(ret));
          return false;
        }
        if (name.find("mediacodec") != std::string::npos) {
          if (rc == RC_CQ) {
            if (q >= 0 && q <= 51) {
              c->global_quality = q;
            }
          }
        }
      }
      break;
    }
  }

  return true;
}
bool set_gpu(void *priv_data, const std::string &name, int gpu) {
  int ret;
  if (gpu < 0)
    return -1;
  if (name.find("nvenc") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "gpu", gpu, 0)) < 0) {
      LOG_ERROR(std::string("nvenc set gpu failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool force_hw(void *priv_data, const std::string &name) {
  int ret;
  if (name.find("_mf") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "hw_encoding", 1, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set hw_encoding failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  if (name.find("videotoolbox") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "allow_sw", 0, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set allow_sw failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool set_others(void *priv_data, const std::string &name) {
  int ret;
  if (name.find("_mf") != std::string::npos) {
    // ff_eAVScenarioInfo_DisplayRemoting = 1
    if ((ret = av_opt_set_int(priv_data, "scenario", 1, 0)) < 0) {
      LOG_ERROR(std::string("mediafoundation set scenario failed, ret = ") +
                av_err2str(ret));
      return false;
    }
  }
  if (name.find("vaapi") != std::string::npos) {
    if ((ret = av_opt_set_int(priv_data, "idr_interval",
                              std::numeric_limits<int>::max(), 0)) < 0) {
      LOG_ERROR(std::string("vaapi set idr_interval failed, ret = ") + av_err2str(ret));
      return false;
    }
  }
  return true;
}

bool change_bit_rate(AVCodecContext *c, const std::string &name, int kbs) {
  if (kbs > 0) {
    c->bit_rate = kbs * 1000;
    if (name.find("qsv") != std::string::npos) {
      c->rc_max_rate = c->bit_rate;
    }
  }
  return true;
}

void vram_encode_test_callback(const uint8_t *data, int32_t len, int32_t key, const void *obj, int64_t pts) {
  (void)data;
  (void)len;
  (void)pts;
  if (obj) {
    int32_t *pkey = (int32_t *)obj;
    *pkey = key;
  }
}

} // namespace util_encode

namespace util_decode {

static bool g_flag_could_not_find_ref_with_poc = false;

bool has_flag_could_not_find_ref_with_poc() {
  bool v = g_flag_could_not_find_ref_with_poc;
  g_flag_could_not_find_ref_with_poc = false;
  return v;
}

} // namespace util_decode

extern "C" void hwcodec_set_flag_could_not_find_ref_with_poc() {
  util_decode::g_flag_could_not_find_ref_with_poc = true;
}
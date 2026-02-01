// https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/hw_decode.c
// https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_video.c

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/log.h>
#include <libavutil/opt.h>
#include <libavutil/pixdesc.h>
}

#include <memory>
#include <stdbool.h>

#define LOG_MODULE "FFMPEG_RAM_DEC"
#include <log.h>
#include <util.h>

#ifdef _WIN32
#include <libavutil/hwcontext_d3d11va.h>
#endif

#include "common.h"
#include "system.h"

// #define CFG_PKG_TRACE

namespace {
typedef void (*RamDecodeCallback)(const void *obj, int width, int height,
                                  enum AVPixelFormat pixfmt,
                                  int linesize[AV_NUM_DATA_POINTERS],
                                  uint8_t *data[AV_NUM_DATA_POINTERS], int key);

class FFmpegRamDecoder {
public:
  AVCodecContext *c_ = NULL;
  AVBufferRef *hw_device_ctx_ = NULL;
  AVFrame *sw_frame_ = NULL;
  AVFrame *frame_ = NULL;
  AVPacket *pkt_ = NULL;
  bool hwaccel_ = true;

  std::string name_;
  AVHWDeviceType device_type_ = AV_HWDEVICE_TYPE_NONE;
  int thread_count_ = 1;
  RamDecodeCallback callback_ = NULL;
  DataFormat data_format_;

#ifdef CFG_PKG_TRACE
  int in_ = 0;
  int out_ = 0;
#endif

  FFmpegRamDecoder(const char *name, int device_type, int thread_count,
                   RamDecodeCallback callback) {
    this->name_ = name;
    this->device_type_ = (AVHWDeviceType)device_type;
    this->thread_count_ = thread_count;
    this->callback_ = callback;
  }

  ~FFmpegRamDecoder() {}

  void free_decoder() {
    if (frame_)
      av_frame_free(&frame_);
    if (pkt_)
      av_packet_free(&pkt_);
    if (sw_frame_)
      av_frame_free(&sw_frame_);
    if (c_)
      avcodec_free_context(&c_);
    if (hw_device_ctx_)
      av_buffer_unref(&hw_device_ctx_);

    frame_ = NULL;
    pkt_ = NULL;
    sw_frame_ = NULL;
    c_ = NULL;
    hw_device_ctx_ = NULL;
  }
  int reset() {
    if (name_.find("h264") != std::string::npos) {
      data_format_ = DataFormat::H264;
    } else if (name_.find("hevc") != std::string::npos) {
      data_format_ = DataFormat::H265;
    } else {
      LOG_ERROR(std::string("unsupported data format:") + name_);
      return -1;
    }
    free_decoder();
    const AVCodec *codec = NULL;
    hwaccel_ = device_type_ != AV_HWDEVICE_TYPE_NONE;
    int ret;
    if (!(codec = avcodec_find_decoder_by_name(name_.c_str()))) {
      LOG_ERROR(std::string("avcodec_find_decoder_by_name ") + name_ + " failed");
      return -1;
    }
    if (!(c_ = avcodec_alloc_context3(codec))) {
      LOG_ERROR(std::string("Could not allocate video codec context"));
      return -1;
    }

    c_->flags |= AV_CODEC_FLAG_LOW_DELAY;
    c_->thread_count =
        device_type_ != AV_HWDEVICE_TYPE_NONE ? 1 : thread_count_;
    c_->thread_type = FF_THREAD_SLICE;

    if (name_.find("qsv") != std::string::npos) {
      if ((ret = av_opt_set(c_->priv_data, "async_depth", "1", 0)) < 0) {
        LOG_ERROR(std::string("qsv set opt async_depth 1 failed"));
        return -1;
      }
      // https://github.com/FFmpeg/FFmpeg/blob/c6364b711bad1fe2fbd90e5b2798f87080ddf5ea/libavcodec/qsvdec.c#L932
      // for disable warning
      c_->pkt_timebase = av_make_q(1, 30);
    }

    if (hwaccel_) {
      ret =
          av_hwdevice_ctx_create(&hw_device_ctx_, device_type_, NULL, NULL, 0);
      if (ret < 0) {
        LOG_ERROR(std::string("av_hwdevice_ctx_create failed, ret = ") + av_err2str(ret));
        return -1;
      }
      c_->hw_device_ctx = av_buffer_ref(hw_device_ctx_);
      if (!check_support()) {
        LOG_ERROR(std::string("check_support failed"));
        return -1;
      }
      if (!(sw_frame_ = av_frame_alloc())) {
        LOG_ERROR(std::string("av_frame_alloc failed"));
        return -1;
      }
    }

    if (!(pkt_ = av_packet_alloc())) {
      LOG_ERROR(std::string("av_packet_alloc failed"));
      return -1;
    }

    if (!(frame_ = av_frame_alloc())) {
      LOG_ERROR(std::string("av_frame_alloc failed"));
      return -1;
    }

    if ((ret = avcodec_open2(c_, codec, NULL)) != 0) {
      LOG_ERROR(std::string("avcodec_open2 failed, ret = ") + av_err2str(ret));
      return -1;
    }
#ifdef CFG_PKG_TRACE
    in_ = 0;
    out_ = 0;
#endif

    return 0;
  }

  int decode(const uint8_t *data, int length, const void *obj) {
    int ret = -1;
#ifdef CFG_PKG_TRACE
    in_++;
    LOG_DEBUG(std::string("delay DI: in:") + in_ + " out:" + out_);
#endif

    if (!data || !length) {
      LOG_ERROR(std::string("illegal decode parameter"));
      return -1;
    }
    pkt_->data = (uint8_t *)data;
    pkt_->size = length;
    ret = do_decode(obj);
    return ret;
  }

private:
  int do_decode(const void *obj) {
    int ret;
    AVFrame *tmp_frame = NULL;
    bool decoded = false;

    ret = avcodec_send_packet(c_, pkt_);
    if (ret < 0) {
      LOG_ERROR(std::string("avcodec_send_packet failed, ret = ") + av_err2str(ret));
      return ret;
    }
    auto start = util::now();
    while (ret >= 0 && util::elapsed_ms(start) < ENCODE_TIMEOUT_MS) {
      if ((ret = avcodec_receive_frame(c_, frame_)) != 0) {
        if (ret != AVERROR(EAGAIN)) {
          LOG_ERROR(std::string("avcodec_receive_frame failed, ret = ") + av_err2str(ret));
        }
        goto _exit;
      }

      if (hwaccel_) {
        if (!frame_->hw_frames_ctx) {
          LOG_ERROR(std::string("hw_frames_ctx is NULL"));
          goto _exit;
        }
        if ((ret = av_hwframe_transfer_data(sw_frame_, frame_, 0)) < 0) {
          LOG_ERROR(std::string("av_hwframe_transfer_data failed, ret = ") +
                    av_err2str(ret));
          goto _exit;
        }

        tmp_frame = sw_frame_;
      } else {
        tmp_frame = frame_;
      }
      decoded = true;
#ifdef CFG_PKG_TRACE
      out_++;
      LOG_DEBUG(std::string("delay DO: in:") + in_ + " out:" + out_);
#endif
#if FF_API_FRAME_KEY
      int key_frame = frame_->flags & AV_FRAME_FLAG_KEY;
#else
      int key_frame = frame_->key_frame;
#endif

      callback_(obj, tmp_frame->width, tmp_frame->height,
                (AVPixelFormat)tmp_frame->format, tmp_frame->linesize,
                tmp_frame->data, key_frame);
    }
  _exit:
    av_packet_unref(pkt_);
    return decoded ? 0 : -1;
  }

  bool check_support() {
#ifdef _WIN32
    if (device_type_ == AV_HWDEVICE_TYPE_D3D11VA) {
      if (!c_->hw_device_ctx) {
        LOG_ERROR(std::string("hw_device_ctx is NULL"));
        return false;
      }
      AVHWDeviceContext *deviceContext =
          (AVHWDeviceContext *)hw_device_ctx_->data;
      if (!deviceContext) {
        LOG_ERROR(std::string("deviceContext is NULL"));
        return false;
      }
      AVD3D11VADeviceContext *d3d11vaDeviceContext =
          (AVD3D11VADeviceContext *)deviceContext->hwctx;
      if (!d3d11vaDeviceContext) {
        LOG_ERROR(std::string("d3d11vaDeviceContext is NULL"));
        return false;
      }
      ID3D11Device *device = d3d11vaDeviceContext->device;
      if (!device) {
        LOG_ERROR(std::string("device is NULL"));
        return false;
      }
      std::unique_ptr<NativeDevice> native_ = std::make_unique<NativeDevice>();
      if (!native_) {
        LOG_ERROR(std::string("Failed to create native device"));
        return false;
      }
      if (!native_->Init(0, (ID3D11Device *)device, 0)) {
        LOG_ERROR(std::string("Failed to init native device"));
        return false;
      }
      if (!native_->support_decode(data_format_)) {
        LOG_ERROR(std::string("Failed to check support ") + name_);
        return false;
      }
      return true;
    } else {
      return true;
    }
#else
    return true;
#endif
  }
};

} // namespace

extern "C" void ffmpeg_ram_free_decoder(FFmpegRamDecoder *decoder) {
  try {
    if (!decoder)
      return;
    decoder->free_decoder();
    delete decoder;
    decoder = NULL;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_free_decoder exception:") + e.what());
  }
}

extern "C" FFmpegRamDecoder *
ffmpeg_ram_new_decoder(const char *name, int device_type, int thread_count,
                       RamDecodeCallback callback) {
  FFmpegRamDecoder *decoder = NULL;
  try {
    decoder = new FFmpegRamDecoder(name, device_type, thread_count, callback);
    if (decoder) {
      if (decoder->reset() == 0) {
        return decoder;
      }
    }
  } catch (std::exception &e) {
    LOG_ERROR(std::string("new decoder exception:") + e.what());
  }
  if (decoder) {
    decoder->free_decoder();
    delete decoder;
    decoder = NULL;
  }
  return NULL;
}

extern "C" int ffmpeg_ram_decode(FFmpegRamDecoder *decoder, const uint8_t *data,
                                 int length, const void *obj) {
  try {
    int ret = decoder->decode(data, length, obj);
    if (DataFormat::H265 == decoder->data_format_ && util_decode::has_flag_could_not_find_ref_with_poc()) {
      return HWCODEC_ERR_HEVC_COULD_NOT_FIND_POC;
    } else {
      return ret == 0 ? HWCODEC_SUCCESS : HWCODEC_ERR_COMMON;
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_decode exception:") + e.what());
  }
  return HWCODEC_ERR_COMMON;
}

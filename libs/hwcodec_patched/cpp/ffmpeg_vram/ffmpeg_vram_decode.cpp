// https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/hw_decode.c
// https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_video.c

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/log.h>
#include <libavutil/opt.h>
#include <libavutil/pixdesc.h>
}
#include <libavutil/hwcontext_d3d11va.h>
#include <memory>
#include <mutex>
#include <stdbool.h>

#include "callback.h"
#include "common.h"
#include "system.h"

#define LOG_MODULE "FFMPEG_VRAM_DEC"
#include <log.h>
#include <util.h>

namespace {

#define USE_SHADER

void lockContext(void *lock_ctx);
void unlockContext(void *lock_ctx);

class FFmpegVRamDecoder {
public:
  AVCodecContext *c_ = NULL;
  AVBufferRef *hw_device_ctx_ = NULL;
  AVCodecParserContext *sw_parser_ctx_ = NULL;
  AVFrame *frame_ = NULL;
  AVPacket *pkt_ = NULL;
  std::unique_ptr<NativeDevice> native_ = nullptr;
  ID3D11Device *d3d11Device_ = NULL;
  ID3D11DeviceContext *d3d11DeviceContext_ = NULL;

  void *device_ = nullptr;
  int64_t luid_ = 0;
  DataFormat dataFormat_;
  std::string name_;
  AVHWDeviceType device_type_ = AV_HWDEVICE_TYPE_D3D11VA;

  bool bt709_ = false;
  bool full_range_ = false;

  FFmpegVRamDecoder(void *device, int64_t luid, DataFormat dataFormat) {
    device_ = device;
    luid_ = luid;
    dataFormat_ = dataFormat;
    switch (dataFormat) {
    case H264:
      name_ = "h264";
      break;
    case H265:
      name_ = "hevc";
      break;
    default:
      LOG_ERROR(std::string("unsupported data format"));
      break;
    }
    // Always use DX11 since it's the only API
    device_type_ = AV_HWDEVICE_TYPE_D3D11VA;
  }

  ~FFmpegVRamDecoder() {}

  void destroy() {
    if (frame_)
      av_frame_free(&frame_);
    if (pkt_)
      av_packet_free(&pkt_);
    if (c_)
      avcodec_free_context(&c_);
    if (hw_device_ctx_) {
      av_buffer_unref(&hw_device_ctx_);
      // AVHWDeviceContext takes ownership of d3d11 object
      d3d11Device_ = nullptr;
      d3d11DeviceContext_ = nullptr;
    } else {
      SAFE_RELEASE(d3d11Device_);
      SAFE_RELEASE(d3d11DeviceContext_);
    }

    frame_ = NULL;
    pkt_ = NULL;
    c_ = NULL;
    hw_device_ctx_ = NULL;
  }
  int reset() {
    destroy();
    if (!native_) {
      native_ = std::make_unique<NativeDevice>();
      if (!native_->Init(luid_, (ID3D11Device *)device_, 4)) {
        LOG_ERROR(std::string("Failed to init native device"));
        return -1;
      }
    }
    if (!native_->support_decode(dataFormat_)) {
      LOG_ERROR(std::string("unsupported data format"));
      return -1;
    }
    d3d11Device_ = native_->device_.Get();
    d3d11Device_->AddRef();
    d3d11DeviceContext_ = native_->context_.Get();
    d3d11DeviceContext_->AddRef();
    const AVCodec *codec = NULL;
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
    hw_device_ctx_ = av_hwdevice_ctx_alloc(device_type_);
    if (!hw_device_ctx_) {
      LOG_ERROR(std::string("av_hwdevice_ctx_create failed"));
      return -1;
    }
    AVHWDeviceContext *deviceContext =
        (AVHWDeviceContext *)hw_device_ctx_->data;
    AVD3D11VADeviceContext *d3d11vaDeviceContext =
        (AVD3D11VADeviceContext *)deviceContext->hwctx;
    d3d11vaDeviceContext->device = d3d11Device_;
    d3d11vaDeviceContext->device_context = d3d11DeviceContext_;
    d3d11vaDeviceContext->lock = lockContext;
    d3d11vaDeviceContext->unlock = unlockContext;
    d3d11vaDeviceContext->lock_ctx = this;
    ret = av_hwdevice_ctx_init(hw_device_ctx_);
    if (ret < 0) {
      LOG_ERROR(std::string("av_hwdevice_ctx_init failed, ret = ") + av_err2str(ret));
      return -1;
    }
    c_->hw_device_ctx = av_buffer_ref(hw_device_ctx_);

    if (!(pkt_ = av_packet_alloc())) {
      LOG_ERROR(std::string("av_packet_alloc failed"));
      return -1;
    }

    if (!(frame_ = av_frame_alloc())) {
      LOG_ERROR(std::string("av_frame_alloc failed"));
      return -1;
    }

    if ((ret = avcodec_open2(c_, codec, NULL)) != 0) {
      LOG_ERROR(std::string("avcodec_open2 failed, ret = ") + av_err2str(ret) +
                ", name=" + name_);
      return -1;
    }

    return 0;
  }

  int decode(const uint8_t *data, int length, DecodeCallback callback,
             const void *obj) {
    int ret = -1;

    if (!data || !length) {
      LOG_ERROR(std::string("illegal decode parameter"));
      return -1;
    }
    pkt_->data = (uint8_t *)data;
    pkt_->size = length;
    ret = do_decode(callback, obj);
    return ret;
  }

private:
  int do_decode(DecodeCallback callback, const void *obj) {
    int ret;
    bool decoded = false;
    bool locked = false;

    ret = avcodec_send_packet(c_, pkt_);
    if (ret < 0) {
      LOG_ERROR(std::string("avcodec_send_packet failed, ret = ") + av_err2str(ret));
      return ret;
    }

    auto start = util::now();
    while (ret >= 0 && util::elapsed_ms(start) < DECODE_TIMEOUT_MS) {
      if ((ret = avcodec_receive_frame(c_, frame_)) != 0) {
        if (ret != AVERROR(EAGAIN)) {
          LOG_ERROR(std::string("avcodec_receive_frame failed, ret = ") + av_err2str(ret));
        }
        goto _exit;
      }
      if (frame_->format != AV_PIX_FMT_D3D11) {
        LOG_ERROR(std::string("only AV_PIX_FMT_D3D11 is supported"));
        goto _exit;
      }
      lockContext(this);
      locked = true;
      if (!convert(frame_, callback, obj)) {
        LOG_ERROR(std::string("Failed to convert"));
        goto _exit;
      }
      if (callback)
        callback(native_->GetCurrentTexture(), obj);
      decoded = true;
    }
  _exit:
    if (locked) {
      unlockContext(this);
    }
    av_packet_unref(pkt_);
    return decoded ? 0 : -1;
  }

  bool convert(AVFrame *frame, DecodeCallback callback, const void *obj) {

    ID3D11Texture2D *texture = (ID3D11Texture2D *)frame->data[0];
    if (!texture) {
      LOG_ERROR(std::string("texture is NULL"));
      return false;
    }
    D3D11_TEXTURE2D_DESC desc2D;
    texture->GetDesc(&desc2D);
    if (desc2D.Format != DXGI_FORMAT_NV12) {
      LOG_ERROR(std::string("only DXGI_FORMAT_NV12 is supported"));
      return false;
    }
    if (!native_->EnsureTexture(frame->width, frame->height)) {
      LOG_ERROR(std::string("Failed to EnsureTexture"));
      return false;
    }
    native_->next(); // comment out to remove picture shaking
#ifdef USE_SHADER
    native_->BeginQuery();
    if (!native_->Nv12ToBgra(frame->width, frame->height, texture,
                             native_->GetCurrentTexture(),
                             (int)frame->data[1])) {
      LOG_ERROR(std::string("Failed to Nv12ToBgra"));
      native_->EndQuery();
      return false;
    }
    native_->EndQuery();
    native_->Query();

#else
    native_->BeginQuery();

    // nv12 -> bgra
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC contentDesc;
    ZeroMemory(&contentDesc, sizeof(contentDesc));
    contentDesc.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
    contentDesc.InputFrameRate.Numerator = 60;
    contentDesc.InputFrameRate.Denominator = 1;
    // TODO: aligned width, height or crop width, height
    contentDesc.InputWidth = frame->width;
    contentDesc.InputHeight = frame->height;
    contentDesc.OutputWidth = frame->width;
    contentDesc.OutputHeight = frame->height;
    contentDesc.OutputFrameRate.Numerator = 60;
    contentDesc.OutputFrameRate.Denominator = 1;
    DXGI_COLOR_SPACE_TYPE colorSpace_out =
        DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709;
    DXGI_COLOR_SPACE_TYPE colorSpace_in;
    if (bt709_) {
      if (full_range_) {
        colorSpace_in = DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709;
      } else {
        colorSpace_in = DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709;
      }
    } else {
      if (full_range_) {
        colorSpace_in = DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601;
      } else {
        colorSpace_in = DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601;
      }
    }
    if (!native_->Process(texture, native_->GetCurrentTexture(), contentDesc,
                          colorSpace_in, colorSpace_out, (int)frame->data[1])) {
      LOG_ERROR(std::string("Failed to process"));
      native_->EndQuery();
      return false;
    }
    native_->context_->Flush();
    native_->EndQuery();
    if (!native_->Query()) {
      LOG_ERROR(std::string("Failed to query"));
      return false;
    }
#endif
    return true;
  }
};

void lockContext(void *lock_ctx) { (void)lock_ctx; }

void unlockContext(void *lock_ctx) { (void)lock_ctx; }

} // namespace

extern "C" int ffmpeg_vram_destroy_decoder(FFmpegVRamDecoder *decoder) {
  try {
    if (!decoder)
      return 0;
    decoder->destroy();
    delete decoder;
    decoder = NULL;
    return 0;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_free_decoder exception:") + e.what());
  }
  return -1;
}

extern "C" FFmpegVRamDecoder *ffmpeg_vram_new_decoder(void *device,
                                                      int64_t luid,
                                                      DataFormat dataFormat) {
  FFmpegVRamDecoder *decoder = NULL;
  try {
    decoder = new FFmpegVRamDecoder(device, luid, dataFormat);
    if (decoder) {
      if (decoder->reset() == 0) {
        return decoder;
      }
    }
  } catch (std::exception &e) {
    LOG_ERROR(std::string("new decoder exception:") + e.what());
  }
  if (decoder) {
    decoder->destroy();
    delete decoder;
    decoder = NULL;
  }
  return NULL;
}

extern "C" int ffmpeg_vram_decode(FFmpegVRamDecoder *decoder,
                                  const uint8_t *data, int length,
                                  DecodeCallback callback, const void *obj) {
  try {
    int ret = decoder->decode(data, length, callback, obj);
    if (DataFormat::H265 == decoder->dataFormat_ && util_decode::has_flag_could_not_find_ref_with_poc()) {
      return HWCODEC_ERR_HEVC_COULD_NOT_FIND_POC;
    } else {
      return ret == 0 ? HWCODEC_SUCCESS : HWCODEC_ERR_COMMON;
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("ffmpeg_ram_decode exception:") + e.what());
  }
  return HWCODEC_ERR_COMMON;
}

extern "C" int ffmpeg_vram_test_decode(int64_t *outLuids, int32_t *outVendors,
                                       int32_t maxDescNum, int32_t *outDescNum,
                                       DataFormat dataFormat,
                                       uint8_t *data, int32_t length,
                                       const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount) {
  try {
    int count = 0;
    struct VendorMapping {
      AdapterVendor adapter_vendor;
      int driver_vendor;
    };
    VendorMapping vendors[] = {
      {ADAPTER_VENDOR_INTEL, VENDOR_INTEL},
      {ADAPTER_VENDOR_NVIDIA, VENDOR_NV},
      {ADAPTER_VENDOR_AMD, VENDOR_AMD}
    };
    
    for (auto vendorMap : vendors) {
      Adapters adapters;
      if (!adapters.Init(vendorMap.adapter_vendor))
        continue;
      for (auto &adapter : adapters.adapters_) {
        int64_t currentLuid = LUID(adapter.get()->desc1_);
        if (util::skip_test(excludedLuids, excludeFormats, excludeCount, currentLuid, dataFormat)) {
          continue;
        }

        FFmpegVRamDecoder *p = (FFmpegVRamDecoder *)ffmpeg_vram_new_decoder(
            nullptr, LUID(adapter.get()->desc1_), dataFormat);
        if (!p)
          continue;
        auto start = util::now();
        bool succ = ffmpeg_vram_decode(p, data, length, nullptr, nullptr) == 0;
        int64_t elapsed = util::elapsed_ms(start);
        if (succ && elapsed < TEST_TIMEOUT_MS) {
          outLuids[count] = LUID(adapter.get()->desc1_);
          outVendors[count] = (int32_t)vendorMap.driver_vendor;  // Map adapter vendor to driver vendor
          count += 1;
        }
        p->destroy();
        delete p;
        p = nullptr;
        if (count >= maxDescNum)
          break;
      }
      if (count >= maxDescNum)
        break;
    }
    *outDescNum = count;
    return 0;
  } catch (const std::exception &e) {
    std::cerr << e.what() << '\n';
  }
  return -1;
}
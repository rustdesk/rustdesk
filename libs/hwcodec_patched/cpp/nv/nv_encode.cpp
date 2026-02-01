#define FFNV_LOG_FUNC
#define FFNV_DEBUG_LOG_FUNC

#include <Samples/NvCodec/NvEncoder/NvEncoderD3D11.h>
#include <Samples/Utils/Logger.h>
#include <Samples/Utils/NvCodecUtils.h>
#include <Samples/Utils/NvEncoderCLIOptions.h>
#include <dynlink_cuda.h>
#include <dynlink_loader.h>
#include <fstream>
#include <iostream>
#include <libavutil/pixfmt.h>
#include <memory>

#include <d3d11.h>
#include <d3d9.h>
#include <wrl/client.h>

using Microsoft::WRL::ComPtr;

#include "callback.h"
#include "common.h"
#include "system.h"
#include "util.h"

#define LOG_MODULE "NVENC"
#include "log.h"

simplelogger::Logger *logger =
    simplelogger::LoggerFactory::CreateConsoleLogger();

namespace {

// #define CONFIG_NV_OPTIMUS_FOR_DEV

#define succ(call) ((call) == 0)

void load_driver(CudaFunctions **pp_cuda_dl, NvencFunctions **pp_nvenc_dl) {
  if (cuda_load_functions(pp_cuda_dl, NULL) < 0) {
    LOG_TRACE(std::string("cuda_load_functions failed"));
    NVENC_THROW_ERROR("cuda_load_functions failed", NV_ENC_ERR_GENERIC);
  }
  if (nvenc_load_functions(pp_nvenc_dl, NULL) < 0) {
    LOG_TRACE(std::string("nvenc_load_functions failed"));
    NVENC_THROW_ERROR("nvenc_load_functions failed", NV_ENC_ERR_GENERIC);
  }
}

void free_driver(CudaFunctions **pp_cuda_dl, NvencFunctions **pp_nvenc_dl) {
  if (*pp_nvenc_dl) {
    nvenc_free_functions(pp_nvenc_dl);
    *pp_nvenc_dl = NULL;
  }
  if (*pp_cuda_dl) {
    cuda_free_functions(pp_cuda_dl);
    *pp_cuda_dl = NULL;
  }
}

class NvencEncoder {
public:
  std::unique_ptr<NativeDevice> native_ = nullptr;
  NvEncoderD3D11 *pEnc_ = nullptr;
  CudaFunctions *cuda_dl_ = nullptr;
  NvencFunctions *nvenc_dl_ = nullptr;

  void *handle_ = nullptr;
  int64_t luid_;
  DataFormat dataFormat_;
  int32_t width_;
  int32_t height_;
  int32_t kbs_;
  int32_t framerate_;
  int32_t gop_;
  bool full_range_ = false;
  bool bt709_ = false;
  NV_ENC_CONFIG encodeConfig_ = {0};

  NvencEncoder(void *handle, int64_t luid, DataFormat dataFormat,
               int32_t width, int32_t height, int32_t kbs, int32_t framerate,
               int32_t gop) {
    handle_ = handle;
    luid_ = luid;
    dataFormat_ = dataFormat;
    width_ = width;
    height_ = height;
    kbs_ = kbs;
    framerate_ = framerate;
    gop_ = gop;

    load_driver(&cuda_dl_, &nvenc_dl_);
  }

  ~NvencEncoder() {}

  bool init() {
    GUID guidCodec;
    switch (dataFormat_) {
    case H264:
      guidCodec = NV_ENC_CODEC_H264_GUID;
      break;
    case H265:
      guidCodec = NV_ENC_CODEC_HEVC_GUID;
      break;
    default:
      LOG_ERROR(std::string("dataFormat not support, dataFormat: ") +
                std::to_string(dataFormat_));
      return false;
    }
    if (!succ(cuda_dl_->cuInit(0))) {
      LOG_TRACE(std::string("cuInit failed"));
      return false;
    }

    native_ = std::make_unique<NativeDevice>();
#ifdef CONFIG_NV_OPTIMUS_FOR_DEV
    if (!native_->Init(luid_, nullptr))
      return false;
#else
    if (!native_->Init(luid_, (ID3D11Device *)handle_)) {
      LOG_ERROR(std::string("d3d device init failed"));
      return false;
    }
#endif

    CUdevice cuDevice = 0;
    if (!succ(cuda_dl_->cuD3D11GetDevice(&cuDevice, native_->adapter_.Get()))) {
      LOG_ERROR(std::string("Failed to get cuDevice"));
      return false;
    }

    int nExtraOutputDelay = 0;
    pEnc_ = new NvEncoderD3D11(cuda_dl_, nvenc_dl_, native_->device_.Get(),
                               width_, height_, NV_ENC_BUFFER_FORMAT_ARGB,
                               nExtraOutputDelay, false, false); // no delay
    NV_ENC_INITIALIZE_PARAMS initializeParams = {0};
    ZeroMemory(&initializeParams, sizeof(initializeParams));
    ZeroMemory(&encodeConfig_, sizeof(encodeConfig_));
    initializeParams.encodeConfig = &encodeConfig_;
    pEnc_->CreateDefaultEncoderParams(
        &initializeParams, guidCodec,
        NV_ENC_PRESET_P3_GUID /*NV_ENC_PRESET_LOW_LATENCY_HP_GUID*/,
        NV_ENC_TUNING_INFO_LOW_LATENCY);

    // no delay
    initializeParams.encodeConfig->frameIntervalP = 1;
    initializeParams.encodeConfig->rcParams.lookaheadDepth = 0;

    // bitrate
    initializeParams.encodeConfig->rcParams.averageBitRate = kbs_ * 1000;
    // framerate
    initializeParams.frameRateNum = framerate_;
    initializeParams.frameRateDen = 1;
    // gop
    initializeParams.encodeConfig->gopLength =
        (gop_ > 0 && gop_ < MAX_GOP) ? gop_ : NVENC_INFINITE_GOPLENGTH;
    // rc method
    initializeParams.encodeConfig->rcParams.rateControlMode =
        NV_ENC_PARAMS_RC_CBR;
    // color
    if (dataFormat_ == H264) {
      setup_h264(initializeParams.encodeConfig);
    } else {
      setup_hevc(initializeParams.encodeConfig);
    }

    pEnc_->CreateEncoder(&initializeParams);
    return true;
  }

  int encode(void *texture, EncodeCallback callback, void *obj, int64_t ms) {
    bool encoded = false;
    std::vector<NvPacket> vPacket;
    const NvEncInputFrame *pEncInput = pEnc_->GetNextInputFrame();

    // TODO: sdk can ensure the inputPtr's width, height same as width_,
    // height_, does capture's frame can ensure width height same with width_,
    // height_ ?
    ID3D11Texture2D *pBgraTextyure =
        reinterpret_cast<ID3D11Texture2D *>(pEncInput->inputPtr);
#ifdef CONFIG_NV_OPTIMUS_FOR_DEV
    copy_texture(texture, pBgraTextyure);
#else
    native_->context_->CopyResource(
        pBgraTextyure, reinterpret_cast<ID3D11Texture2D *>(texture));
#endif

    NV_ENC_PIC_PARAMS picParams = {0};
    picParams.inputTimeStamp = ms;
    pEnc_->EncodeFrame(vPacket);
    for (NvPacket &packet : vPacket) {
      int32_t key = (packet.pictureType == NV_ENC_PIC_TYPE_IDR ||
                     packet.pictureType == NV_ENC_PIC_TYPE_I)
                        ? 1
                        : 0;
      if (packet.data.size() > 0) {
        if (callback)
          callback(packet.data.data(), packet.data.size(), key, obj, ms);
        encoded = true;
      }
    }
    return encoded ? 0 : -1;
  }

  void destroy() {
    if (pEnc_) {
      pEnc_->DestroyEncoder();
      delete pEnc_;
      pEnc_ = nullptr;
    }
    free_driver(&cuda_dl_, &nvenc_dl_);
  }

  void setup_h264(NV_ENC_CONFIG *encodeConfig) {
    NV_ENC_CODEC_CONFIG *encodeCodecConfig = &encodeConfig->encodeCodecConfig;
    NV_ENC_CONFIG_H264 *h264 = &encodeCodecConfig->h264Config;
    NV_ENC_CONFIG_H264_VUI_PARAMETERS *vui = &h264->h264VUIParameters;
    vui->videoFullRangeFlag = !!full_range_;
    vui->colourMatrix = bt709_ ? NV_ENC_VUI_MATRIX_COEFFS_BT709 : NV_ENC_VUI_MATRIX_COEFFS_SMPTE170M;
    vui->colourPrimaries = bt709_ ? NV_ENC_VUI_COLOR_PRIMARIES_BT709 : NV_ENC_VUI_COLOR_PRIMARIES_SMPTE170M;
    vui->transferCharacteristics =
        bt709_ ? NV_ENC_VUI_TRANSFER_CHARACTERISTIC_BT709 : NV_ENC_VUI_TRANSFER_CHARACTERISTIC_SMPTE170M;
    vui->colourDescriptionPresentFlag = 1;
    vui->videoSignalTypePresentFlag = 1;

    h264->sliceMode = 3;
    h264->sliceModeData = 1;
    h264->repeatSPSPPS = 1;
    // Specifies the chroma format. Should be set to 1 for yuv420 input, 3 for
    // yuv444 input
    h264->chromaFormatIDC = 1;
    h264->level = NV_ENC_LEVEL_AUTOSELECT;

    encodeConfig->profileGUID = NV_ENC_H264_PROFILE_MAIN_GUID;
  }

  void setup_hevc(NV_ENC_CONFIG *encodeConfig) {
    NV_ENC_CODEC_CONFIG *encodeCodecConfig = &encodeConfig->encodeCodecConfig;
    NV_ENC_CONFIG_HEVC *hevc = &encodeCodecConfig->hevcConfig;
    NV_ENC_CONFIG_HEVC_VUI_PARAMETERS *vui = &hevc->hevcVUIParameters;
    vui->videoFullRangeFlag = !!full_range_;
    vui->colourMatrix = bt709_ ? NV_ENC_VUI_MATRIX_COEFFS_BT709 : NV_ENC_VUI_MATRIX_COEFFS_SMPTE170M;
    vui->colourPrimaries = bt709_ ? NV_ENC_VUI_COLOR_PRIMARIES_BT709 : NV_ENC_VUI_COLOR_PRIMARIES_SMPTE170M;
    vui->transferCharacteristics =
        bt709_ ? NV_ENC_VUI_TRANSFER_CHARACTERISTIC_BT709 : NV_ENC_VUI_TRANSFER_CHARACTERISTIC_SMPTE170M;
    vui->colourDescriptionPresentFlag = 1;
    vui->videoSignalTypePresentFlag = 1;

    hevc->sliceMode = 3;
    hevc->sliceModeData = 1;
    hevc->repeatSPSPPS = 1;
    // Specifies the chroma format. Should be set to 1 for yuv420 input, 3 for
    // yuv444 input
    hevc->chromaFormatIDC = 1;
    hevc->level = NV_ENC_LEVEL_AUTOSELECT;
    hevc->outputPictureTimingSEI = 1;
    hevc->tier = NV_ENC_TIER_HEVC_MAIN;

    encodeConfig->profileGUID = NV_ENC_HEVC_PROFILE_MAIN_GUID;
  }

private:
#ifdef CONFIG_NV_OPTIMUS_FOR_DEV
  int copy_texture(void *src, void *dst) {
    ComPtr<ID3D11Device> src_device = (ID3D11Device *)handle_;
    ComPtr<ID3D11DeviceContext> src_deviceContext;
    src_device->GetImmediateContext(src_deviceContext.ReleaseAndGetAddressOf());
    ComPtr<ID3D11Texture2D> src_tex = (ID3D11Texture2D *)src;
    ComPtr<ID3D11Texture2D> dst_tex = (ID3D11Texture2D *)dst;
    HRESULT hr;

    D3D11_TEXTURE2D_DESC desc;
    ZeroMemory(&desc, sizeof(desc));
    src_tex->GetDesc(&desc);
    desc.Usage = D3D11_USAGE_STAGING;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    desc.BindFlags = 0;
    desc.MiscFlags = 0;
    ComPtr<ID3D11Texture2D> staging_tex;
    src_device->CreateTexture2D(&desc, NULL,
                                staging_tex.ReleaseAndGetAddressOf());
    src_deviceContext->CopyResource(staging_tex.Get(), src_tex.Get());

    D3D11_MAPPED_SUBRESOURCE map;
    src_deviceContext->Map(staging_tex.Get(), 0, D3D11_MAP_READ, 0, &map);
    std::unique_ptr<uint8_t[]> buffer(
        new uint8_t[desc.Width * desc.Height * 4]);
    memcpy(buffer.get(), map.pData, desc.Width * desc.Height * 4);
    src_deviceContext->Unmap(staging_tex.Get(), 0);

    D3D11_BOX Box;
    Box.left = 0;
    Box.right = desc.Width;
    Box.top = 0;
    Box.bottom = desc.Height;
    Box.front = 0;
    Box.back = 1;
    native_->context_->UpdateSubresource(dst_tex.Get(), 0, &Box, buffer.get(),
                                         desc.Width * 4,
                                         desc.Width * desc.Height * 4);

    return 0;
  }
#endif
};

} // namespace

extern "C" {

int nv_encode_driver_support() {
  try {
    CudaFunctions *cuda_dl = NULL;
    NvencFunctions *nvenc_dl = NULL;
    load_driver(&cuda_dl, &nvenc_dl);
    free_driver(&cuda_dl, &nvenc_dl);
    return 0;
  } catch (const std::exception &e) {
    LOG_TRACE(std::string("driver not support, ") + e.what());
  }
  return -1;
}

int nv_destroy_encoder(void *encoder) {
  try {
    NvencEncoder *e = (NvencEncoder *)encoder;
    if (e) {
      e->destroy();
      delete e;
      e = NULL;
    }
    return 0;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("destroy failed: ") + e.what());
  }
  return -1;
}

void *nv_new_encoder(void *handle, int64_t luid, DataFormat dataFormat,
                     int32_t width, int32_t height, int32_t kbs,
                     int32_t framerate, int32_t gop) {
  NvencEncoder *e = NULL;
  try {
    e = new NvencEncoder(handle, luid, dataFormat, width, height, kbs,
                         framerate, gop);
    if (!e->init()) {
      goto _exit;
    }
    return e;
  } catch (const std::exception &ex) {
    LOG_ERROR(std::string("new failed: ") + ex.what());
    goto _exit;
  }

_exit:
  if (e) {
    e->destroy();
    delete e;
    e = NULL;
  }
  return NULL;
}

int nv_encode(void *encoder, void *texture, EncodeCallback callback, void *obj,
              int64_t ms) {
  try {
    NvencEncoder *e = (NvencEncoder *)encoder;
    return e->encode(texture, callback, obj, ms);
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("encode failed: ") + e.what());
  }
  return -1;
}

// ref: Reconfigure API

#define RECONFIGURE_HEAD                                                       \
  NvencEncoder *enc = (NvencEncoder *)e;                                       \
  NV_ENC_CONFIG sEncodeConfig = {0};                                           \
  NV_ENC_INITIALIZE_PARAMS sInitializeParams = {0};                            \
  sInitializeParams.encodeConfig = &sEncodeConfig;                             \
  enc->pEnc_->GetInitializeParams(&sInitializeParams);                         \
  NV_ENC_RECONFIGURE_PARAMS params = {0};                                      \
  params.version = NV_ENC_RECONFIGURE_PARAMS_VER;                              \
  params.reInitEncodeParams = sInitializeParams;

#define RECONFIGURE_TAIL                                                       \
  if (enc->pEnc_->Reconfigure(&params)) {                                      \
    return 0;                                                                  \
  }

int nv_test_encode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum, int32_t *outDescNum,
                   DataFormat dataFormat, int32_t width,
                   int32_t height, int32_t kbs, int32_t framerate,
                   int32_t gop, const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount) {
  try {
    Adapters adapters;
    if (!adapters.Init(ADAPTER_VENDOR_NVIDIA))
      return -1;
    int count = 0;
    for (auto &adapter : adapters.adapters_) {
      int64_t currentLuid = LUID(adapter.get()->desc1_);
      if (util::skip_test(excludedLuids, excludeFormats, excludeCount, currentLuid, dataFormat)) {
        continue;
      }

      NvencEncoder *e = (NvencEncoder *)nv_new_encoder(
          (void *)adapter.get()->device_.Get(), currentLuid,
          dataFormat, width, height, kbs, framerate, gop);
      if (!e)
        continue;
      if (e->native_->EnsureTexture(e->width_, e->height_)) {
        e->native_->next();
        int32_t key_obj = 0;
        auto start = util::now();
        bool succ = nv_encode(e, e->native_->GetCurrentTexture(), util_encode::vram_encode_test_callback, &key_obj,
                      0) == 0 && key_obj == 1;
        int64_t elapsed = util::elapsed_ms(start);
        if (succ && elapsed < TEST_TIMEOUT_MS) {
          outLuids[count] = currentLuid;
          outVendors[count] = VENDOR_NV;
          count += 1;
        }
      }
      e->destroy();
      delete e;
      e = nullptr;
      if (count >= maxDescNum)
        break;
    }
    *outDescNum = count;
    return 0;

  } catch (const std::exception &e) {
    LOG_ERROR(std::string("test failed: ") + e.what());
  }
  return -1;
}

int nv_set_bitrate(void *e, int32_t kbs) {
  try {
    RECONFIGURE_HEAD
    params.reInitEncodeParams.encodeConfig->rcParams.averageBitRate =
        kbs * 1000;
    RECONFIGURE_TAIL
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("set bitrate to ") + std::to_string(kbs) +
              "k failed: " + e.what());
  }
  return -1;
}

int nv_set_framerate(void *e, int32_t framerate) {
  try {
    RECONFIGURE_HEAD
    params.reInitEncodeParams.frameRateNum = framerate;
    params.reInitEncodeParams.frameRateDen = 1;
    RECONFIGURE_TAIL
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("set framerate failed: ") + e.what());
  }
  return -1;
}
} // extern "C"

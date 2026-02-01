#include <public/common/AMFFactory.h>
#include <public/common/AMFSTL.h>
#include <public/common/ByteArray.h>
#include <public/common/Thread.h>
#include <public/common/TraceAdapter.h>
#include <public/include/components/VideoConverter.h>
#include <public/include/components/VideoDecoderUVD.h>

#include <cstring>
#include <iostream>

#include "callback.h"
#include "common.h"
#include "system.h"
#include "util.h"

#define LOG_MODULE "AMFDEC"
#include "log.h"

#define AMF_FACILITY L"AMFDecoder"

#define AMF_CHECK_RETURN(res, msg)                                             \
  if (res != AMF_OK) {                                                         \
    LOG_ERROR(std::string(msg) + ", result code: " + std::to_string(int(res)));             \
    return res;                                                                \
  }

namespace {
class AMFDecoder {

private:
  // system
  void *device_;
  int64_t luid_;
  std::unique_ptr<NativeDevice> nativeDevice_ = nullptr;
  // amf
  AMFFactoryHelper AMFFactory_;
  amf::AMFContextPtr AMFContext_ = NULL;
  amf::AMFComponentPtr AMFDecoder_ = NULL;
  amf::AMF_MEMORY_TYPE AMFMemoryType_;
  amf::AMF_SURFACE_FORMAT decodeFormatOut_ = amf::AMF_SURFACE_NV12;
  amf::AMF_SURFACE_FORMAT textureFormatOut_;
  amf::AMFComponentPtr AMFConverter_ = NULL;
  int last_width_ = 0;
  int last_height_ = 0;
  amf_wstring codec_;
  bool full_range_ = false;
  bool bt709_ = false;

  // buffer
  std::vector<std::vector<uint8_t>> buffer_;

public:
  AMFDecoder(void *device, int64_t luid, amf::AMF_MEMORY_TYPE memoryTypeOut,
             amf_wstring codec, amf::AMF_SURFACE_FORMAT textureFormatOut) {
    device_ = device;
    luid_ = luid;
    AMFMemoryType_ = memoryTypeOut;
    textureFormatOut_ = textureFormatOut;
    codec_ = codec;
  }

  ~AMFDecoder() {}

  AMF_RESULT decode(uint8_t *iData, uint32_t iDataSize, DecodeCallback callback,
                    void *obj) {
    AMF_RESULT res = AMF_FAIL;
    bool decoded = false;
    amf::AMFBufferPtr iDataWrapBuffer = NULL;

    res = AMFContext_->CreateBufferFromHostNative(iData, iDataSize,
                                                  &iDataWrapBuffer, NULL);
    AMF_CHECK_RETURN(res, "CreateBufferFromHostNative failed");
    res = AMFDecoder_->SubmitInput(iDataWrapBuffer);
    if (res == AMF_RESOLUTION_CHANGED) {
      iDataWrapBuffer = NULL;
      LOG_INFO(std::string("resolution changed"));
      res = AMFDecoder_->Drain();
      AMF_CHECK_RETURN(res, "Drain failed");
      res = AMFDecoder_->Terminate();
      AMF_CHECK_RETURN(res, "Terminate failed");
      res = AMFDecoder_->Init(decodeFormatOut_, 0, 0);
      AMF_CHECK_RETURN(res, "Init failed");
      res = AMFContext_->CreateBufferFromHostNative(iData, iDataSize,
                                                    &iDataWrapBuffer, NULL);
      AMF_CHECK_RETURN(res, "CreateBufferFromHostNative failed");
      res = AMFDecoder_->SubmitInput(iDataWrapBuffer);
    }
    AMF_CHECK_RETURN(res, "SubmitInput failed");
    amf::AMFDataPtr oData = NULL;
    auto start = util::now();
    do {
      res = AMFDecoder_->QueryOutput(&oData);
      if (res == AMF_REPEAT) {
        amf_sleep(1);
      }
    } while (res == AMF_REPEAT && util::elapsed_ms(start) < DECODE_TIMEOUT_MS);
    if (res == AMF_OK && oData != NULL) {
      amf::AMFSurfacePtr surface(oData);
      AMF_RETURN_IF_INVALID_POINTER(surface, L"surface is NULL");

      if (surface->GetPlanesCount() == 0)
        return AMF_FAIL;

      // convert texture
      amf::AMFDataPtr convertData;
      res = Convert(surface, convertData);
      AMF_CHECK_RETURN(res, "Convert failed");
      amf::AMFSurfacePtr convertSurface(convertData);
      if (!convertSurface || convertSurface->GetPlanesCount() == 0)
        return AMF_FAIL;

      // For DirectX objects, when a pointer to a COM interface is returned,
      // GetNative does not call IUnknown::AddRef on the interface being
      // returned.
      void *native = convertSurface->GetPlaneAt(0)->GetNative();
      if (!native)
        return AMF_FAIL;
      switch (convertSurface->GetMemoryType()) {
      case amf::AMF_MEMORY_DX11: {
        {
          ID3D11Texture2D *src = (ID3D11Texture2D *)native;
          D3D11_TEXTURE2D_DESC desc;
          src->GetDesc(&desc);
          nativeDevice_->EnsureTexture(desc.Width, desc.Height);
          nativeDevice_->next();
          ID3D11Texture2D *dst = nativeDevice_->GetCurrentTexture();
          nativeDevice_->context_->CopyResource(dst, src);
          nativeDevice_->context_->Flush();
          if (callback)
            callback(dst, obj);
          decoded = true;
        }
        break;
      } break;
      case amf::AMF_MEMORY_OPENCL: {
        uint8_t *buf = (uint8_t *)native;
      } break;
      }

      surface = NULL;
      convertData = NULL;
      convertSurface = NULL;
    }
    oData = NULL;
    iDataWrapBuffer = NULL;
    return decoded ? AMF_OK : AMF_FAIL;
    return AMF_OK;
  }

  AMF_RESULT destroy() {
    // Terminate converter before terminate decoder get "[AMFDeviceDX11Impl]
    // Warning: Possible memory leak detected: DX11 device is being destroyed,
    // but has 6 surfaces associated with it. This is OK if there are references
    // to the device outside AMF"

    if (AMFConverter_ != NULL) {
      AMFConverter_->Drain();
      AMFConverter_->Terminate();
      AMFConverter_ = NULL;
    }
    if (AMFDecoder_ != NULL) {
      AMFDecoder_->Drain();
      AMFDecoder_->Terminate();
      AMFDecoder_ = NULL;
    }
    if (AMFContext_ != NULL) {
      AMFContext_->Terminate();
      AMFContext_ = NULL; // context is the last
    }
    AMFFactory_.Terminate();
    return AMF_OK;
  }

  AMF_RESULT initialize() {
    AMF_RESULT res;

    res = AMFFactory_.Init();
    AMF_CHECK_RETURN(res, "AMFFactory Init failed");
    amf::AMFSetCustomTracer(AMFFactory_.GetTrace());
    amf::AMFTraceEnableWriter(AMF_TRACE_WRITER_CONSOLE, true);
    amf::AMFTraceSetWriterLevel(AMF_TRACE_WRITER_CONSOLE, AMF_TRACE_WARNING);

    res = AMFFactory_.GetFactory()->CreateContext(&AMFContext_);
    AMF_CHECK_RETURN(res, "CreateContext failed");

    switch (AMFMemoryType_) {
    case amf::AMF_MEMORY_DX11:
      nativeDevice_ = std::make_unique<NativeDevice>();
      if (!nativeDevice_->Init(luid_, (ID3D11Device *)device_, 4)) {
        LOG_ERROR(std::string("Init NativeDevice failed"));
        return AMF_FAIL;
      }
      res = AMFContext_->InitDX11(
          nativeDevice_->device_.Get()); // can be DX11 device
      AMF_CHECK_RETURN(res, "InitDX11 failed");
      break;
    default:
      LOG_ERROR(std::string("unsupported memory type: ") +
                std::to_string((int)AMFMemoryType_));
      return AMF_FAIL;
    }

    res = AMFFactory_.GetFactory()->CreateComponent(AMFContext_, codec_.c_str(),
                                                    &AMFDecoder_);
    AMF_CHECK_RETURN(res, "CreateComponent failed");

    res = setParameters();
    AMF_CHECK_RETURN(res, "setParameters failed");

    res = AMFDecoder_->Init(decodeFormatOut_, 0, 0);
    AMF_CHECK_RETURN(res, "Init decoder failed");

    return AMF_OK;
  }

private:
  AMF_RESULT setParameters() {
    AMF_RESULT res;
    res =
        AMFDecoder_->SetProperty(AMF_TIMESTAMP_MODE, amf_int64(AMF_TS_DECODE));
    AMF_RETURN_IF_FAILED(
        res, L"SetProperty AMF_TIMESTAMP_MODE to AMF_TS_DECODE failed");
    res =
        AMFDecoder_->SetProperty(AMF_VIDEO_DECODER_REORDER_MODE,
                                 amf_int64(AMF_VIDEO_DECODER_MODE_LOW_LATENCY));
    AMF_CHECK_RETURN(res, "SetProperty AMF_VIDEO_DECODER_REORDER_MODE failed");
    // color
    res = AMFDecoder_->SetProperty<amf_int64>(
        AMF_VIDEO_DECODER_COLOR_RANGE,
        full_range_ ? AMF_COLOR_RANGE_FULL : AMF_COLOR_RANGE_STUDIO);
    AMF_CHECK_RETURN(res, "SetProperty AMF_VIDEO_DECODER_COLOR_RANGE failed");
    res = AMFDecoder_->SetProperty<amf_int64>(
        AMF_VIDEO_DECODER_COLOR_PROFILE,
        bt709_ ? (full_range_ ? AMF_VIDEO_CONVERTER_COLOR_PROFILE_FULL_709
                              : AMF_VIDEO_CONVERTER_COLOR_PROFILE_709)
               : (full_range_ ? AMF_VIDEO_CONVERTER_COLOR_PROFILE_FULL_601
                              : AMF_VIDEO_CONVERTER_COLOR_PROFILE_601));
    AMF_CHECK_RETURN(res, "SetProperty AMF_VIDEO_DECODER_COLOR_PROFILE failed");
    // res = AMFDecoder_->SetProperty<amf_int64>(
    //     AMF_VIDEO_DECODER_COLOR_TRANSFER_CHARACTERISTIC,
    //     bt709_ ? AMF_COLOR_TRANSFER_CHARACTERISTIC_BT709
    //            : AMF_COLOR_TRANSFER_CHARACTERISTIC_SMPTE170M);
    // AMF_CHECK_RETURN(
    //     res,
    //     "SetProperty AMF_VIDEO_DECODER_COLOR_TRANSFER_CHARACTERISTIC
    //     failed");
    // res = AMFDecoder_->SetProperty<amf_int64>(
    //     AMF_VIDEO_DECODER_COLOR_PRIMARIES,
    //     bt709_ ? AMF_COLOR_PRIMARIES_BT709 : AMF_COLOR_PRIMARIES_SMPTE170M);
    // AMF_CHECK_RETURN(res,
    //                  "SetProperty AMF_VIDEO_DECODER_COLOR_PRIMARIES failed");
    return AMF_OK;
  }

  AMF_RESULT Convert(IN amf::AMFSurfacePtr &surface,
                     OUT amf::AMFDataPtr &convertData) {
    if (decodeFormatOut_ == textureFormatOut_)
      return AMF_OK;
    AMF_RESULT res;

    int width = surface->GetPlaneAt(0)->GetWidth();
    int height = surface->GetPlaneAt(0)->GetHeight();
    if (AMFConverter_ != NULL) {
      if (width != last_width_ || height != last_height_) {
        LOG_INFO(std::string("Convert size changed, (") + std::to_string(last_width_) + "x" +
                 std::to_string(last_height_) + ") -> (" +
                 std::to_string(width) + "x" + std::to_string(width) + ")");
        AMFConverter_->Terminate();
        AMFConverter_ = NULL;
      }
    }
    if (!AMFConverter_) {
      res = AMFFactory_.GetFactory()->CreateComponent(
          AMFContext_, AMFVideoConverter, &AMFConverter_);
      AMF_CHECK_RETURN(res, "Convert CreateComponent failed");
      res = AMFConverter_->SetProperty(AMF_VIDEO_CONVERTER_MEMORY_TYPE,
                                       AMFMemoryType_);
      AMF_CHECK_RETURN(res,
                       "SetProperty AMF_VIDEO_CONVERTER_MEMORY_TYPE failed");
      res = AMFConverter_->SetProperty(AMF_VIDEO_CONVERTER_OUTPUT_FORMAT,
                                       textureFormatOut_);
      AMF_CHECK_RETURN(res,
                       "SetProperty AMF_VIDEO_CONVERTER_OUTPUT_FORMAT failed");
      res = AMFConverter_->SetProperty(AMF_VIDEO_CONVERTER_OUTPUT_SIZE,
                                       ::AMFConstructSize(width, height));
      AMF_CHECK_RETURN(res,
                       "SetProperty AMF_VIDEO_CONVERTER_OUTPUT_SIZE failed");
      res = AMFConverter_->Init(decodeFormatOut_, width, height);
      AMF_CHECK_RETURN(res, "Init converter failed");
      // color
      res = AMFConverter_->SetProperty<amf_int64>(
          AMF_VIDEO_CONVERTER_INPUT_COLOR_RANGE,
          full_range_ ? AMF_COLOR_RANGE_FULL : AMF_COLOR_RANGE_STUDIO);
      AMF_CHECK_RETURN(
          res, "SetProperty AMF_VIDEO_CONVERTER_INPUT_COLOR_RANGE failed");
      res = AMFConverter_->SetProperty<amf_int64>(
          AMF_VIDEO_CONVERTER_OUTPUT_COLOR_RANGE, AMF_COLOR_RANGE_FULL);
      AMF_CHECK_RETURN(
          res, "SetProperty AMF_VIDEO_CONVERTER_OUTPUT_COLOR_RANGE failed");
      res = AMFConverter_->SetProperty<amf_int64>(
          AMF_VIDEO_CONVERTER_COLOR_PROFILE,
          bt709_ ? (full_range_ ? AMF_VIDEO_CONVERTER_COLOR_PROFILE_FULL_709
                                : AMF_VIDEO_CONVERTER_COLOR_PROFILE_709)
                 : (full_range_ ? AMF_VIDEO_CONVERTER_COLOR_PROFILE_FULL_601
                                : AMF_VIDEO_CONVERTER_COLOR_PROFILE_601));
      AMF_CHECK_RETURN(res,
                       "SetProperty AMF_VIDEO_CONVERTER_COLOR_PROFILE failed");
      res = AMFConverter_->SetProperty<amf_int64>(
          AMF_VIDEO_CONVERTER_INPUT_TRANSFER_CHARACTERISTIC,
          bt709_ ? AMF_COLOR_TRANSFER_CHARACTERISTIC_BT709
                 : AMF_COLOR_TRANSFER_CHARACTERISTIC_SMPTE170M);
      AMF_CHECK_RETURN(
          res, "SetProperty AMF_VIDEO_CONVERTER_INPUT_TRANSFER_CHARACTERISTIC "
               "failed");
      res = AMFConverter_->SetProperty<amf_int64>(
          AMF_VIDEO_CONVERTER_INPUT_COLOR_PRIMARIES,
          bt709_ ? AMF_COLOR_PRIMARIES_BT709 : AMF_COLOR_PRIMARIES_SMPTE170M);
      AMF_CHECK_RETURN(
          res, "SetProperty AMF_VIDEO_CONVERTER_INPUT_COLOR_PRIMARIES failed");
    }
    last_width_ = width;
    last_height_ = height;
    res = AMFConverter_->SubmitInput(surface);
    AMF_CHECK_RETURN(res, "Convert SubmitInput failed");
    res = AMFConverter_->QueryOutput(&convertData);
    AMF_CHECK_RETURN(res, "Convert QueryOutput failed");
    return AMF_OK;
  }
};

bool convert_codec(DataFormat lhs, amf_wstring &rhs) {
  switch (lhs) {
  case H264:
    rhs = AMFVideoDecoderUVD_H264_AVC;
    break;
  case H265:
    rhs = AMFVideoDecoderHW_H265_HEVC;
    break;
  default:
    LOG_ERROR(std::string("unsupported codec: ") + std::to_string(lhs));
    return false;
  }
  return true;
}

} // namespace

#include "amf_common.cpp"

extern "C" {

int amf_destroy_decoder(void *decoder) {
  try {
    AMFDecoder *dec = (AMFDecoder *)decoder;
    if (dec) {
      dec->destroy();
      delete dec;
      dec = NULL;
      return 0;
    }
  } catch (const std::exception &e) {
          LOG_ERROR(std::string("destroy failed: ") + e.what());
  }
  return -1;
}

void *amf_new_decoder(void *device, int64_t luid,
                      DataFormat dataFormat) {
  AMFDecoder *dec = NULL;
  try {
    amf_wstring codecStr;
    amf::AMF_MEMORY_TYPE memory;
    amf::AMF_SURFACE_FORMAT surfaceFormat;
    if (!convert_api(memory)) {
      return NULL;
    }
    if (!convert_codec(dataFormat, codecStr)) {
      return NULL;
    }
    dec = new AMFDecoder(device, luid, memory, codecStr, amf::AMF_SURFACE_BGRA);
    if (dec) {
      if (dec->initialize() == AMF_OK) {
        return dec;
      }
    }
  } catch (const std::exception &e) {
          LOG_ERROR(std::string("new failed: ") + e.what());
  }
  if (dec) {
    dec->destroy();
    delete dec;
    dec = NULL;
  }
  return NULL;
}

int amf_decode(void *decoder, uint8_t *data, int32_t length,
               DecodeCallback callback, void *obj) {
  try {
    AMFDecoder *dec = (AMFDecoder *)decoder;
    if (dec->decode(data, length, callback, obj) == AMF_OK) {
      return HWCODEC_SUCCESS;
    }
  } catch (const std::exception &e) {
          LOG_ERROR(std::string("decode failed: ") + e.what());
  }
  return HWCODEC_ERR_COMMON;
}

int amf_test_decode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum,
                    int32_t *outDescNum, DataFormat dataFormat,
                    uint8_t *data, int32_t length, const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount) {
  try {
    Adapters adapters;
    if (!adapters.Init(ADAPTER_VENDOR_AMD))
      return -1;
    int count = 0;
    for (auto &adapter : adapters.adapters_) {
      int64_t currentLuid = LUID(adapter.get()->desc1_);
      if (util::skip_test(excludedLuids, excludeFormats, excludeCount, currentLuid, dataFormat)) {
        continue;
      }
      
      AMFDecoder *p = (AMFDecoder *)amf_new_decoder(
          nullptr, currentLuid, dataFormat);
      if (!p)
        continue;
      auto start = util::now();
      bool succ = p->decode(data, length, nullptr, nullptr) == AMF_OK;
      int64_t elapsed = util::elapsed_ms(start);
      if (succ && elapsed < TEST_TIMEOUT_MS) {
        outLuids[count] = currentLuid;
        outVendors[count] = VENDOR_AMD;
        count += 1;
      }
      p->destroy();
      delete p;
      p = nullptr;
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

} // extern "C"

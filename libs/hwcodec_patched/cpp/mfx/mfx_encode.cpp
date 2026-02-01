#include <cstring>
#include <iostream>
#include <libavutil/pixfmt.h>
#include <limits>
#include <sample_defs.h>
#include <sample_utils.h>

#include "callback.h"
#include "common.h"
#include "system.h"
#include "util.h"

#define LOG_MODULE "MFXENC"
#include "log.h"

// #define CONFIG_USE_VPP
#define CONFIG_USE_D3D_CONVERT

#define CHECK_STATUS(X, MSG)                                                   \
  {                                                                            \
    mfxStatus __sts = (X);                                                     \
    if (__sts != MFX_ERR_NONE) {                                               \
      LOG_ERROR(std::string(MSG) + " failed, sts=" + std::to_string((int)__sts));           \
      return __sts;                                                            \
    }                                                                          \
  }

namespace {

mfxStatus MFX_CDECL simple_getHDL(mfxHDL pthis, mfxMemId mid, mfxHDL *handle) {
  mfxHDLPair *pair = (mfxHDLPair *)handle;
  pair->first = mid;
  pair->second = (mfxHDL)(UINT)0;
  return MFX_ERR_NONE;
}

mfxFrameAllocator frameAllocator{{},   NULL,          NULL, NULL,
                                 NULL, simple_getHDL, NULL};

mfxStatus InitSession(MFXVideoSession &session) {
  mfxInitParam mfxparams{};
  mfxIMPL impl = MFX_IMPL_HARDWARE_ANY | MFX_IMPL_VIA_D3D11;
  mfxparams.Implementation = impl;
  mfxparams.Version.Major = 1;
  mfxparams.Version.Minor = 0;
  mfxparams.GPUCopy = MFX_GPUCOPY_OFF;

  return session.InitEx(mfxparams);
}


class VplEncoder {
public:
  std::unique_ptr<NativeDevice> native_ = nullptr;
  MFXVideoSession session_;
  MFXVideoENCODE *mfxENC_ = nullptr;
  std::vector<mfxFrameSurface1> encSurfaces_;
  std::vector<mfxU8> bstData_;
  mfxBitstream mfxBS_;
  mfxVideoParam mfxEncParams_;
  mfxExtBuffer *extbuffers_[4] = {NULL, NULL, NULL, NULL};
  mfxExtCodingOption coding_option_;
  mfxExtCodingOption2 coding_option2_;
  mfxExtCodingOption3 coding_option3_;
  mfxExtVideoSignalInfo signal_info_;
  ComPtr<ID3D11Texture2D> nv12Texture_ = nullptr;

// vpp
#ifdef CONFIG_USE_VPP
  MFXVideoVPP *mfxVPP_ = nullptr;
  mfxVideoParam vppParams_;
  mfxExtBuffer *vppExtBuffers_[1] = {NULL};
  mfxExtVPPDoNotUse vppDontUse_;
  mfxU32 vppDontUseArgList_[4];
  std::vector<mfxFrameSurface1> vppSurfaces_;
#endif

  void *handle_ = nullptr;
  int64_t luid_;
  DataFormat dataFormat_;
  int32_t width_ = 0;
  int32_t height_ = 0;
  int32_t kbs_;
  int32_t framerate_;
  int32_t gop_;

  bool full_range_ = false;
  bool bt709_ = false;

  VplEncoder(void *handle, int64_t luid, DataFormat dataFormat,
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
  }

  ~VplEncoder() {}

  mfxStatus Reset() {
    mfxStatus sts = MFX_ERR_NONE;

    if (!native_) {
      native_ = std::make_unique<NativeDevice>();
      if (!native_->Init(luid_, (ID3D11Device *)handle_)) {
        LOG_ERROR(std::string("failed to init native device"));
        return MFX_ERR_DEVICE_FAILED;
      }
    }
    sts = resetMFX();
    CHECK_STATUS(sts, "resetMFX");
#ifdef CONFIG_USE_VPP
    sts = resetVpp();
    CHECK_STATUS(sts, "resetVpp");
#endif
    sts = resetEnc();
    CHECK_STATUS(sts, "resetEnc");
    return MFX_ERR_NONE;
  }

  int encode(ID3D11Texture2D *tex, EncodeCallback callback, void *obj,
             int64_t ms) {
    mfxStatus sts = MFX_ERR_NONE;

    int nEncSurfIdx =
        GetFreeSurfaceIndex(encSurfaces_.data(), encSurfaces_.size());
    if (nEncSurfIdx >= encSurfaces_.size()) {
      LOG_ERROR(std::string("no free enc surface"));
      return -1;
    }
    mfxFrameSurface1 *encSurf = &encSurfaces_[nEncSurfIdx];
#ifdef CONFIG_USE_VPP
    mfxSyncPoint syncp;
    sts = vppOneFrame(tex, encSurf, syncp);
    syncp = NULL;
    if (sts != MFX_ERR_NONE) {
      LOG_ERROR(std::string("vppOneFrame failed, sts=") + std::to_string((int)sts));
      return -1;
    }
#elif defined(CONFIG_USE_D3D_CONVERT)
    DXGI_COLOR_SPACE_TYPE colorSpace_in =
        DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709;
    DXGI_COLOR_SPACE_TYPE colorSpace_out;
    if (bt709_) {
      if (full_range_) {
        colorSpace_out = DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709;
      } else {
        colorSpace_out = DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709;
      }
    } else {
      if (full_range_) {
        colorSpace_out = DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601;
      } else {
        colorSpace_out = DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601;
      }
    }
    if (!nv12Texture_) {
      D3D11_TEXTURE2D_DESC desc;
      ZeroMemory(&desc, sizeof(desc));
      tex->GetDesc(&desc);
      desc.Format = DXGI_FORMAT_NV12;
      desc.MiscFlags = 0;
      HRI(native_->device_->CreateTexture2D(
          &desc, NULL, nv12Texture_.ReleaseAndGetAddressOf()));
    }
    if (!native_->BgraToNv12(tex, nv12Texture_.Get(), width_, height_,
                             colorSpace_in, colorSpace_out)) {
      LOG_ERROR(std::string("failed to convert to NV12"));
      return -1;
    }
    encSurf->Data.MemId = nv12Texture_.Get();
#else
    encSurf->Data.MemId = tex;
#endif
    return encodeOneFrame(encSurf, callback, obj, ms);
  }

  void destroy() {
    if (mfxENC_) {
      //  - It is recommended to close Media SDK components first, before
      //  releasing allocated surfaces, since
      //    some surfaces may still be locked by internal Media SDK resources.
      mfxENC_->Close();
      delete mfxENC_;
      mfxENC_ = NULL;
    }
#ifdef CONFIG_USE_VPP
    if (mfxVPP_) {
      mfxVPP_->Close();
      delete mfxVPP_;
      mfxVPP_ = NULL;
    }
#endif
    // session closed automatically on destruction
  }

private:
  mfxStatus resetMFX() {
    mfxStatus sts = MFX_ERR_NONE;

    sts = InitSession(session_);
    CHECK_STATUS(sts, "InitSession");
    sts = session_.SetHandle(MFX_HANDLE_D3D11_DEVICE, native_->device_.Get());
    CHECK_STATUS(sts, "SetHandle");
    sts = session_.SetFrameAllocator(&frameAllocator);
    CHECK_STATUS(sts, "SetFrameAllocator");

    return MFX_ERR_NONE;
  }

#ifdef CONFIG_USE_VPP
  mfxStatus resetVpp() {
    mfxStatus sts = MFX_ERR_NONE;
    memset(&vppParams_, 0, sizeof(vppParams_));
    vppParams_.IOPattern =
        MFX_IOPATTERN_IN_VIDEO_MEMORY | MFX_IOPATTERN_OUT_VIDEO_MEMORY;
    vppParams_.vpp.In.PicStruct = MFX_PICSTRUCT_PROGRESSIVE;
    vppParams_.vpp.In.FrameRateExtN = framerate_;
    vppParams_.vpp.In.FrameRateExtD = 1;
    vppParams_.vpp.In.Width = MSDK_ALIGN16(width_);
    vppParams_.vpp.In.Height =
        (MFX_PICSTRUCT_PROGRESSIVE == vppParams_.vpp.In.PicStruct)
            ? MSDK_ALIGN16(height_)
            : MSDK_ALIGN32(height_);
    vppParams_.vpp.In.CropX = 0;
    vppParams_.vpp.In.CropY = 0;
    vppParams_.vpp.In.CropW = width_;
    vppParams_.vpp.In.CropH = height_;
    vppParams_.vpp.In.Shift = 0;
    memcpy(&vppParams_.vpp.Out, &vppParams_.vpp.In, sizeof(vppParams_.vpp.Out));
    vppParams_.vpp.In.FourCC = MFX_FOURCC_RGB4;
    vppParams_.vpp.Out.FourCC = MFX_FOURCC_NV12;
    vppParams_.vpp.In.ChromaFormat = MFX_CHROMAFORMAT_YUV444;
    vppParams_.vpp.Out.ChromaFormat = MFX_CHROMAFORMAT_YUV420;
    vppParams_.AsyncDepth = 1;

    vppParams_.ExtParam = vppExtBuffers_;
    vppParams_.NumExtParam = 1;
    vppExtBuffers_[0] = (mfxExtBuffer *)&vppDontUse_;
    vppDontUse_.Header.BufferId = MFX_EXTBUFF_VPP_DONOTUSE;
    vppDontUse_.Header.BufferSz = sizeof(vppDontUse_);
    vppDontUse_.AlgList = vppDontUseArgList_;
    vppDontUse_.NumAlg = 4;
    vppDontUseArgList_[0] = MFX_EXTBUFF_VPP_DENOISE;
    vppDontUseArgList_[1] = MFX_EXTBUFF_VPP_SCENE_ANALYSIS;
    vppDontUseArgList_[2] = MFX_EXTBUFF_VPP_DETAIL;
    vppDontUseArgList_[3] = MFX_EXTBUFF_VPP_PROCAMP;

    if (mfxVPP_) {
      mfxVPP_->Close();
      delete mfxVPP_;
      mfxVPP_ = NULL;
    }
    mfxVPP_ = new MFXVideoVPP(session_);
    if (!mfxVPP_) {
      LOG_ERROR(std::string("Failed to create MFXVideoVPP"));
      return MFX_ERR_MEMORY_ALLOC;
    }

    sts = mfxVPP_->Query(&vppParams_, &vppParams_);
    CHECK_STATUS(sts, "vpp query");
    mfxFrameAllocRequest vppAllocRequest;
    ZeroMemory(&vppAllocRequest, sizeof(vppAllocRequest));
    memcpy(&vppAllocRequest.Info, &vppParams_.vpp.In, sizeof(mfxFrameInfo));
    sts = mfxVPP_->QueryIOSurf(&vppParams_, &vppAllocRequest);
    CHECK_STATUS(sts, "vpp QueryIOSurf");

    vppSurfaces_.resize(vppAllocRequest.NumFrameSuggested);
    for (int i = 0; i < vppAllocRequest.NumFrameSuggested; i++) {
      memset(&vppSurfaces_[i], 0, sizeof(mfxFrameSurface1));
      memcpy(&vppSurfaces_[i].Info, &vppParams_.vpp.In, sizeof(mfxFrameInfo));
    }

    sts = mfxVPP_->Init(&vppParams_);
    MSDK_IGNORE_MFX_STS(sts, MFX_WRN_PARTIAL_ACCELERATION);
    CHECK_STATUS(sts, "vpp init");

    return MFX_ERR_NONE;
  }
#endif

  mfxStatus resetEnc() {
    mfxStatus sts = MFX_ERR_NONE;
    memset(&mfxEncParams_, 0, sizeof(mfxEncParams_));

    // Basic
    if (!convert_codec(dataFormat_, mfxEncParams_.mfx.CodecId)) {
      LOG_ERROR(std::string("unsupported dataFormat: ") + std::to_string(dataFormat_));
      return MFX_ERR_UNSUPPORTED;
    }
    // mfxEncParams_.mfx.LowPower = MFX_CODINGOPTION_ON;
    mfxEncParams_.mfx.BRCParamMultiplier = 0;

    // Frame Info
    mfxEncParams_.mfx.FrameInfo.FrameRateExtN = framerate_;
    mfxEncParams_.mfx.FrameInfo.FrameRateExtD = 1;
#ifdef CONFIG_USE_VPP
    mfxEncParams_.mfx.FrameInfo.FourCC = MFX_FOURCC_NV12;
    mfxEncParams_.mfx.FrameInfo.ChromaFormat = MFX_CHROMAFORMAT_YUV420;
#elif defined(CONFIG_USE_D3D_CONVERT)
    mfxEncParams_.mfx.FrameInfo.FourCC = MFX_FOURCC_NV12;
    mfxEncParams_.mfx.FrameInfo.ChromaFormat = MFX_CHROMAFORMAT_YUV420;
#else
    mfxEncParams_.mfx.FrameInfo.FourCC = MFX_FOURCC_BGR4;
    mfxEncParams_.mfx.FrameInfo.ChromaFormat = MFX_CHROMAFORMAT_YUV444;
#endif
    mfxEncParams_.mfx.FrameInfo.BitDepthLuma = 8;
    mfxEncParams_.mfx.FrameInfo.BitDepthChroma = 8;
    mfxEncParams_.mfx.FrameInfo.Shift = 0;
    mfxEncParams_.mfx.FrameInfo.PicStruct = MFX_PICSTRUCT_PROGRESSIVE;
    mfxEncParams_.mfx.FrameInfo.CropX = 0;
    mfxEncParams_.mfx.FrameInfo.CropY = 0;
    mfxEncParams_.mfx.FrameInfo.CropW = width_;
    mfxEncParams_.mfx.FrameInfo.CropH = height_;
    // Width must be a multiple of 16
    // Height must be a multiple of 16 in case of frame picture and a multiple
    // of 32 in case of field picture
    mfxEncParams_.mfx.FrameInfo.Width = MSDK_ALIGN16(width_);
    mfxEncParams_.mfx.FrameInfo.Height =
        (MFX_PICSTRUCT_PROGRESSIVE == mfxEncParams_.mfx.FrameInfo.PicStruct)
            ? MSDK_ALIGN16(height_)
            : MSDK_ALIGN32(height_);
    
    // Encoding Options
    mfxEncParams_.mfx.EncodedOrder = 0;

    mfxEncParams_.IOPattern = MFX_IOPATTERN_IN_VIDEO_MEMORY;

    // Configuration for low latency
    mfxEncParams_.AsyncDepth = 1; // 1 is best for low latency
    mfxEncParams_.mfx.GopRefDist =
        1; // 1 is best for low latency, I and P frames only
    mfxEncParams_.mfx.GopPicSize = (gop_ > 0 && gop_ < 0xFFFF) ? gop_ : 0xFFFF;
    // quality
    // https://www.intel.com/content/www/us/en/developer/articles/technical/common-bitrate-control-methods-in-intel-media-sdk.html
    mfxEncParams_.mfx.TargetUsage = MFX_TARGETUSAGE_BEST_SPEED;
    mfxEncParams_.mfx.RateControlMethod = MFX_RATECONTROL_VBR;
    mfxEncParams_.mfx.InitialDelayInKB = 0;
    mfxEncParams_.mfx.BufferSizeInKB = 512;
    mfxEncParams_.mfx.TargetKbps = kbs_;
    mfxEncParams_.mfx.MaxKbps = kbs_;
    mfxEncParams_.mfx.NumSlice = 1;
    mfxEncParams_.mfx.NumRefFrame = 0;

    if (H264 == dataFormat_) {
      mfxEncParams_.mfx.CodecLevel = MFX_LEVEL_AVC_51;
      mfxEncParams_.mfx.CodecProfile = MFX_PROFILE_AVC_MAIN;
    } else if (H265 == dataFormat_) {
      mfxEncParams_.mfx.CodecLevel = MFX_LEVEL_HEVC_51;
      mfxEncParams_.mfx.CodecProfile = MFX_PROFILE_HEVC_MAIN;
    }

    resetEncExtParams();

    // Create Media SDK encoder
    if (mfxENC_) {
      mfxENC_->Close();
      delete mfxENC_;
      mfxENC_ = NULL;
    }
    mfxENC_ = new MFXVideoENCODE(session_);
    if (!mfxENC_) {
      LOG_ERROR(std::string("failed to create MFXVideoENCODE"));
      return MFX_ERR_NOT_INITIALIZED;
    }

    // Validate video encode parameters (optional)
    // - In this example the validation result is written to same structure
    // - MFX_WRN_INCOMPATIBLE_VIDEO_PARAM is returned if some of the video
    // parameters are not supported,
    //   instead the encoder will select suitable parameters closest matching
    //   the requested configuration
    sts = mfxENC_->Query(&mfxEncParams_, &mfxEncParams_);
    MSDK_IGNORE_MFX_STS(sts, MFX_WRN_INCOMPATIBLE_VIDEO_PARAM);
    CHECK_STATUS(sts, "Query");

    mfxFrameAllocRequest EncRequest;
    memset(&EncRequest, 0, sizeof(EncRequest));
    sts = mfxENC_->QueryIOSurf(&mfxEncParams_, &EncRequest);
    CHECK_STATUS(sts, "QueryIOSurf");

    // Allocate surface headers (mfxFrameSurface1) for encoder
    encSurfaces_.resize(EncRequest.NumFrameSuggested);
    for (int i = 0; i < EncRequest.NumFrameSuggested; i++) {
      memset(&encSurfaces_[i], 0, sizeof(mfxFrameSurface1));
      memcpy(&encSurfaces_[i].Info, &mfxEncParams_.mfx.FrameInfo,
             sizeof(mfxFrameInfo));
    }

    // Initialize the Media SDK encoder
    sts = mfxENC_->Init(&mfxEncParams_);
    CHECK_STATUS(sts, "Init");

    // Retrieve video parameters selected by encoder.
    // - BufferSizeInKB parameter is required to set bit stream buffer size
    sts = mfxENC_->GetVideoParam(&mfxEncParams_);
    CHECK_STATUS(sts, "GetVideoParam");

    // Prepare Media SDK bit stream buffer
    memset(&mfxBS_, 0, sizeof(mfxBS_));
    mfxBS_.MaxLength = mfxEncParams_.mfx.BufferSizeInKB * 1024;
    bstData_.resize(mfxBS_.MaxLength);
    mfxBS_.Data = bstData_.data();

    return MFX_ERR_NONE;
  }

#ifdef CONFIG_USE_VPP
  mfxStatus vppOneFrame(void *texture, mfxFrameSurface1 *out,
                        mfxSyncPoint syncp) {
    mfxStatus sts = MFX_ERR_NONE;

    int surfIdx =
        GetFreeSurfaceIndex(vppSurfaces_.data(),
                            vppSurfaces_.size()); // Find free frame surface
    if (surfIdx >= vppSurfaces_.size()) {
      LOG_ERROR(std::string("No free vpp surface"));
      return MFX_ERR_MORE_SURFACE;
    }
    mfxFrameSurface1 *in = &vppSurfaces_[surfIdx];
    in->Data.MemId = texture;

    for (;;) {
      sts = mfxVPP_->RunFrameVPPAsync(in, out, NULL, &syncp);

      if (MFX_ERR_NONE < sts &&
          !syncp) // repeat the call if warning and no output
      {
        if (MFX_WRN_DEVICE_BUSY == sts)
          MSDK_SLEEP(1); // wait if device is busy
      } else if (MFX_ERR_NONE < sts && syncp) {
        sts = MFX_ERR_NONE; // ignore warnings if output is available
        break;
      } else {
        break; // not a warning
      }
    }

    if (MFX_ERR_NONE == sts) {
      sts = session_.SyncOperation(
          syncp, 1000); // Synchronize. Wait until encoded frame is ready
      CHECK_STATUS(sts, "SyncOperation");
    }

    return sts;
  }
#endif

  int encodeOneFrame(mfxFrameSurface1 *in, EncodeCallback callback, void *obj,
                     int64_t ms) {
    mfxStatus sts = MFX_ERR_NONE;
    mfxSyncPoint syncp;
    bool encoded = false;

    auto start = util::now();
    do {
      if (util::elapsed_ms(start) > ENCODE_TIMEOUT_MS) {
        LOG_ERROR(std::string("encode timeout"));
        break;
      }
      mfxBS_.DataLength = 0;
      mfxBS_.DataOffset = 0;
      mfxBS_.TimeStamp = ms * 90; // ms to 90KHZ
      mfxBS_.DecodeTimeStamp = mfxBS_.TimeStamp;
      sts = mfxENC_->EncodeFrameAsync(NULL, in, &mfxBS_, &syncp);
      if (MFX_ERR_NONE == sts) {
        if (!syncp) {
          LOG_ERROR(std::string("should not happen, error is none while syncp is null"));
          break;
        }
        sts = session_.SyncOperation(
            syncp, 1000); // Synchronize. Wait until encoded frame is ready
        if (MFX_ERR_NONE != sts) {
          LOG_ERROR(std::string("SyncOperation failed, sts=") + std::to_string(sts));
          break;
        }
        if (mfxBS_.DataLength <= 0) {
          LOG_ERROR(std::string("mfxBS_.DataLength <= 0"));
          break;
        }
        int key = (mfxBS_.FrameType & MFX_FRAMETYPE_I) ||
                  (mfxBS_.FrameType & MFX_FRAMETYPE_IDR);
        if (callback)
          callback(mfxBS_.Data + mfxBS_.DataOffset, mfxBS_.DataLength, key, obj,
                   ms);
        encoded = true;
        break;
      } else if (MFX_WRN_DEVICE_BUSY == sts) {
        LOG_INFO(std::string("device busy"));
        Sleep(1);
        continue;
      } else if (MFX_ERR_NOT_ENOUGH_BUFFER == sts) {
        LOG_ERROR(std::string("not enough buffer, size=") +
                  std::to_string(mfxBS_.MaxLength));
        if (mfxBS_.MaxLength < 10 * 1024 * 1024) {
          mfxBS_.MaxLength *= 2;
          bstData_.resize(mfxBS_.MaxLength);
          mfxBS_.Data = bstData_.data();
          Sleep(1);
          continue;
        } else {
          break;
        }
      } else {
        LOG_ERROR(std::string("EncodeFrameAsync failed, sts=") + std::to_string(sts));
        break;
      }
      // double confirm, check continue
    } while (MFX_WRN_DEVICE_BUSY == sts || MFX_ERR_NOT_ENOUGH_BUFFER == sts);

    if (!encoded) {
      LOG_ERROR(std::string("encode failed, sts=") + std::to_string(sts));
    }
    return encoded ? 0 : -1;
  }

  void resetEncExtParams() {
    // coding option
    memset(&coding_option_, 0, sizeof(mfxExtCodingOption));
    coding_option_.Header.BufferId = MFX_EXTBUFF_CODING_OPTION;
    coding_option_.Header.BufferSz = sizeof(mfxExtCodingOption);
    coding_option_.NalHrdConformance = MFX_CODINGOPTION_OFF;
    extbuffers_[0] = (mfxExtBuffer *)&coding_option_;

    // coding option2
    memset(&coding_option2_, 0, sizeof(mfxExtCodingOption2));
    coding_option2_.Header.BufferId = MFX_EXTBUFF_CODING_OPTION2;
    coding_option2_.Header.BufferSz = sizeof(mfxExtCodingOption2);
    coding_option2_.RepeatPPS = MFX_CODINGOPTION_OFF;
    extbuffers_[1] = (mfxExtBuffer *)&coding_option2_;

    // coding option3
    memset(&coding_option3_, 0, sizeof(mfxExtCodingOption3));
    coding_option3_.Header.BufferId = MFX_EXTBUFF_CODING_OPTION3;
    coding_option3_.Header.BufferSz = sizeof(mfxExtCodingOption3);
    extbuffers_[2] = (mfxExtBuffer *)&coding_option3_;
    
    // signal info
    memset(&signal_info_, 0, sizeof(mfxExtVideoSignalInfo));
    signal_info_.Header.BufferId = MFX_EXTBUFF_VIDEO_SIGNAL_INFO;
    signal_info_.Header.BufferSz = sizeof(mfxExtVideoSignalInfo);
    signal_info_.VideoFormat = 5;
    signal_info_.ColourDescriptionPresent = 1;
    signal_info_.VideoFullRange = !!full_range_;
    signal_info_.MatrixCoefficients =
        bt709_ ? AVCOL_SPC_BT709 : AVCOL_SPC_SMPTE170M;
    signal_info_.ColourPrimaries =
        bt709_ ? AVCOL_PRI_BT709 : AVCOL_PRI_SMPTE170M;
    signal_info_.TransferCharacteristics =
        bt709_ ? AVCOL_TRC_BT709 : AVCOL_TRC_SMPTE170M;
    // https://github.com/GStreamer/gstreamer/blob/651dcb49123ec516e7c582e4a49a5f3f15c10f93/subprojects/gst-plugins-bad/sys/qsv/gstqsvh264enc.cpp#L1647
    extbuffers_[3] = (mfxExtBuffer *)&signal_info_;

    mfxEncParams_.ExtParam = extbuffers_;
    mfxEncParams_.NumExtParam = 4;
  }

  bool convert_codec(DataFormat dataFormat, mfxU32 &CodecId) {
    switch (dataFormat) {
    case H264:
      CodecId = MFX_CODEC_AVC;
      return true;
    case H265:
      CodecId = MFX_CODEC_HEVC;
      return true;
    }
    return false;
  }
};

} // namespace

extern "C" {

int mfx_driver_support() {
  MFXVideoSession session;
  return InitSession(session) == MFX_ERR_NONE ? 0 : -1;
}

int mfx_destroy_encoder(void *encoder) {
  VplEncoder *p = (VplEncoder *)encoder;
  if (p) {
    p->destroy();
    delete p;
    p = NULL;
  }
  return 0;
}

void *mfx_new_encoder(void *handle, int64_t luid,
                      DataFormat dataFormat, int32_t w, int32_t h, int32_t kbs,
                      int32_t framerate, int32_t gop) {
  VplEncoder *p = NULL;
  try {
    p = new VplEncoder(handle, luid, dataFormat, w, h, kbs, framerate,
                       gop);
    if (!p) {
      return NULL;
    }
    mfxStatus sts = p->Reset();
    if (sts == MFX_ERR_NONE) {
      return p;
    } else {
      LOG_ERROR(std::string("Init failed, sts=") + std::to_string(sts));
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("Exception: ") + e.what());
  }

  if (p) {
    p->destroy();
    delete p;
    p = NULL;
  }
  return NULL;
}

int mfx_encode(void *encoder, ID3D11Texture2D *tex, EncodeCallback callback,
               void *obj, int64_t ms) {
  try {
    return ((VplEncoder *)encoder)->encode(tex, callback, obj, ms);
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("Exception: ") + e.what());
  }
  return -1;
}

int mfx_test_encode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum, int32_t *outDescNum,
                    DataFormat dataFormat, int32_t width,
                    int32_t height, int32_t kbs, int32_t framerate,
                    int32_t gop, const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount) {
  try {
    Adapters adapters;
    if (!adapters.Init(ADAPTER_VENDOR_INTEL))
      return -1;
    int count = 0;
    for (auto &adapter : adapters.adapters_) {
      int64_t currentLuid = LUID(adapter.get()->desc1_);
      if (util::skip_test(excludedLuids, excludeFormats, excludeCount, currentLuid, dataFormat)) {
        continue;
      }
      
      VplEncoder *e = (VplEncoder *)mfx_new_encoder(
          (void *)adapter.get()->device_.Get(), currentLuid,
          dataFormat, width, height, kbs, framerate, gop);
      if (!e)
        continue;
      if (e->native_->EnsureTexture(e->width_, e->height_)) {
        e->native_->next();
        int32_t key_obj = 0;
        auto start = util::now();
        bool succ = mfx_encode(e, e->native_->GetCurrentTexture(), util_encode::vram_encode_test_callback, &key_obj,
                       0) == 0 && key_obj == 1;
        int64_t elapsed = util::elapsed_ms(start);
        if (succ && elapsed < TEST_TIMEOUT_MS) {
          outLuids[count] = currentLuid;
          outVendors[count] = VENDOR_INTEL;
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

// https://github.com/Intel-Media-SDK/MediaSDK/blob/master/doc/mediasdk-man.md#dynamic-bitrate-change
// https://github.com/Intel-Media-SDK/MediaSDK/blob/master/doc/mediasdk-man.md#mfxinfomfx
// https://spec.oneapi.io/onevpl/2.4.0/programming_guide/VPL_prg_encoding.html#configuration-change
int mfx_set_bitrate(void *encoder, int32_t kbs) {
  try {
    VplEncoder *p = (VplEncoder *)encoder;
    mfxStatus sts = MFX_ERR_NONE;
    // https://github.com/GStreamer/gstreamer/blob/e19428a802c2f4ee9773818aeb0833f93509a1c0/subprojects/gst-plugins-bad/sys/qsv/gstqsvencoder.cpp#L1312
    p->kbs_ = kbs;
    p->mfxENC_->GetVideoParam(&p->mfxEncParams_);
    p->mfxEncParams_.mfx.TargetKbps = kbs;
    p->mfxEncParams_.mfx.MaxKbps = kbs;
    sts = p->mfxENC_->Reset(&p->mfxEncParams_);
    if (sts != MFX_ERR_NONE) {
      LOG_ERROR(std::string("reset failed, sts=") + std::to_string(sts));
      return -1;
    }
    return 0;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("Exception: ") + e.what());
  }
  return -1;
}

int mfx_set_framerate(void *encoder, int32_t framerate) {
  LOG_WARN("not support change framerate");
  return -1;
}
}

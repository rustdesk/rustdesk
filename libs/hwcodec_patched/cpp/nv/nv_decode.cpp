#define FFNV_LOG_FUNC
#define FFNV_DEBUG_LOG_FUNC

#include <DirectXMath.h>
#include <Samples/NvCodec/NvDecoder/NvDecoder.h>
#include <Samples/Utils/NvCodecUtils.h>
#include <algorithm>
#include <array>
#include <d3dcompiler.h>
#include <directxcolors.h>
#include <iostream>
#include <libavutil/pixfmt.h>
#include <thread>

#include "callback.h"
#include "common.h"
#include "system.h"
#include "util.h"

#define LOG_MODULE "CUVID"
#include "log.h"

#define NUMVERTICES 6

using namespace DirectX;

namespace {

#define succ(call) ((call) == 0)

class CUVIDAutoUnmapper {
  CudaFunctions *cudl_ = NULL;
  CUgraphicsResource *pCuResource_ = NULL;

public:
  CUVIDAutoUnmapper(CudaFunctions *cudl, CUgraphicsResource *pCuResource)
      : cudl_(cudl), pCuResource_(pCuResource) {
    if (!succ(cudl->cuGraphicsMapResources(1, pCuResource, 0))) {
      LOG_TRACE(std::string("cuGraphicsMapResources failed"));
      NVDEC_THROW_ERROR("cuGraphicsMapResources failed", CUDA_ERROR_UNKNOWN);
    }
  }
  ~CUVIDAutoUnmapper() {
    if (!succ(cudl_->cuGraphicsUnmapResources(1, pCuResource_, 0))) {
      LOG_TRACE(std::string("cuGraphicsUnmapResources failed"));
      // NVDEC_THROW_ERROR("cuGraphicsUnmapResources failed",
      // CUDA_ERROR_UNKNOWN);
    }
  }
};

class CUVIDAutoCtxPopper {
  CudaFunctions *cudl_ = NULL;

public:
  CUVIDAutoCtxPopper(CudaFunctions *cudl, CUcontext cuContext) : cudl_(cudl) {
    if (!succ(cudl->cuCtxPushCurrent(cuContext))) {
      LOG_TRACE(std::string("cuCtxPushCurrent failed"));
      NVDEC_THROW_ERROR("cuCtxPopCurrent failed", CUDA_ERROR_UNKNOWN);
    }
  }
  ~CUVIDAutoCtxPopper() {
    if (!succ(cudl_->cuCtxPopCurrent(NULL))) {
      LOG_TRACE(std::string("cuCtxPopCurrent failed"));
      // NVDEC_THROW_ERROR("cuCtxPopCurrent failed", CUDA_ERROR_UNKNOWN);
    }
  }
};

void load_driver(CudaFunctions **pp_cudl, CuvidFunctions **pp_cvdl) {
  if (cuda_load_functions(pp_cudl, NULL) < 0) {
    LOG_TRACE(std::string("cuda_load_functions failed"));
    NVDEC_THROW_ERROR("cuda_load_functions failed", CUDA_ERROR_UNKNOWN);
  }
  if (cuvid_load_functions(pp_cvdl, NULL) < 0) {
    LOG_TRACE(std::string("cuvid_load_functions failed"));
    NVDEC_THROW_ERROR("cuvid_load_functions failed", CUDA_ERROR_UNKNOWN);
  }
}

void free_driver(CudaFunctions **pp_cudl, CuvidFunctions **pp_cvdl) {
  if (*pp_cvdl) {
    cuvid_free_functions(pp_cvdl);
    *pp_cvdl = NULL;
  }
  if (*pp_cudl) {
    cuda_free_functions(pp_cudl);
    *pp_cudl = NULL;
  }
}

typedef struct _VERTEX {
  DirectX::XMFLOAT3 Pos;
  DirectX::XMFLOAT2 TexCoord;
} VERTEX;

class CuvidDecoder {
public:
  CudaFunctions *cudl_ = NULL;
  CuvidFunctions *cvdl_ = NULL;
  NvDecoder *dec_ = NULL;
  CUcontext cuContext_ = NULL;
  CUgraphicsResource cuResource_[2] = {NULL, NULL}; // r8, r8g8
  ComPtr<ID3D11Texture2D> textures_[2] = {NULL, NULL};
  ComPtr<ID3D11RenderTargetView> RTV_ = NULL;
  ComPtr<ID3D11ShaderResourceView> SRV_[2] = {NULL, NULL};
  ComPtr<ID3D11VertexShader> vertexShader_ = NULL;
  ComPtr<ID3D11PixelShader> pixelShader_ = NULL;
  ComPtr<ID3D11SamplerState> samplerLinear_ = NULL;
  std::unique_ptr<NativeDevice> native_ = nullptr;

  void *device_;
  int64_t luid_;
  DataFormat dataFormat_;

  bool prepare_tried_ = false;
  bool prepare_ok_ = false;

  int width_ = 0;
  int height_ = 0;
  CUVIDEOFORMAT last_video_format_ = {};

public:
  CuvidDecoder(void *device, int64_t luid, DataFormat dataFormat) {
    device_ = device;
    luid_ = luid;
    dataFormat_ = dataFormat;
    ZeroMemory(&last_video_format_, sizeof(last_video_format_));
    load_driver(&cudl_, &cvdl_);
  }

  ~CuvidDecoder() {}

  bool init() {
    if (!succ(cudl_->cuInit(0))) {
      LOG_ERROR(std::string("cuInit failed"));
      return false;
    }
    CUdevice cuDevice = 0;
    native_ = std::make_unique<NativeDevice>();
    if (!native_->Init(luid_, (ID3D11Device *)device_, 4)) {
      LOG_ERROR(std::string("Failed to init native device"));
      return false;
    }
    if (!succ(cudl_->cuD3D11GetDevice(&cuDevice, native_->adapter_.Get()))) {
      LOG_ERROR(std::string("Failed to get cuDevice"));
      return false;
    }

    if (!succ(cudl_->cuCtxCreate(&cuContext_, 0, cuDevice))) {
      LOG_ERROR(std::string("Failed to create cuContext"));
      return false;
    }
    if (!create_nvdecoder()) {
      LOG_ERROR(std::string("Failed to create nvdecoder"));
      return false;
    }
    return true;
  }

  // ref: HandlePictureDisplay
  int decode(uint8_t *data, int len, DecodeCallback callback, void *obj) {
    int nFrameReturned = decode_and_recreate(data, len);
    if (nFrameReturned == -2) {
      nFrameReturned = dec_->Decode(data, len, CUVID_PKT_ENDOFPICTURE);
    }
    if (nFrameReturned <= 0) {
      return -1;
    }
    last_video_format_ = dec_->GetLatestVideoFormat();
    cudaVideoSurfaceFormat format = dec_->GetOutputFormat();
    int width = dec_->GetWidth();
    int height = dec_->GetHeight();
    if (prepare_tried_ && (width != width_ || height != height_)) {
      LOG_INFO(std::string("resolution changed, (") + std::to_string(width_) + "," +
               std::to_string(height_) + ") -> (" + std::to_string(width) +
               "," + std::to_string(height) + ")");
      reset_prepare();
      width_ = width;
      height_ = height;
    }
    if (!prepare()) {
      LOG_ERROR(std::string("prepare failed"));
      return -1;
    }
    bool decoded = false;
    for (int i = 0; i < nFrameReturned; i++) {
      uint8_t *pFrame = dec_->GetFrame();
      native_->BeginQuery();
      if (!copy_cuda_frame(pFrame)) {
        LOG_ERROR(std::string("copy_cuda_frame failed"));
        native_->EndQuery();
        return -1;
      }
      if (!native_->EnsureTexture(width, height)) {
        LOG_ERROR(std::string("EnsureTexture failed"));
        native_->EndQuery();
        return -1;
      }
      native_->next();
      if (!set_rtv(native_->GetCurrentTexture())) {
        LOG_ERROR(std::string("set_rtv failed"));
        native_->EndQuery();
        return -1;
      }
      if (!draw()) {
        LOG_ERROR(std::string("draw failed"));
        native_->EndQuery();
        return -1;
      }
      native_->EndQuery();
      if (!native_->Query()) {
        LOG_ERROR(std::string("Query failed"));
      }

      if (callback)
        callback(native_->GetCurrentTexture(), obj);
      decoded = true;
    }
    return decoded ? 0 : -1;
  }

  void destroy() {
    if (dec_) {
      delete dec_;
      dec_ = nullptr;
    }
    if (cudl_ && cuContext_) {
      cudl_->cuCtxPushCurrent(cuContext_);
      for (int i = 0; i < 2; i++) {
        if (cuResource_[i]) {
          cudl_->cuGraphicsUnregisterResource(cuResource_[i]);
          cuResource_[i] = NULL;
        }
      }
      cudl_->cuCtxPopCurrent(NULL);
      cudl_->cuCtxDestroy(cuContext_);
      cuContext_ = NULL;
    }
    free_driver(&cudl_, &cvdl_);
  }

private:
  void reset_prepare() {
    prepare_tried_ = false;
    prepare_ok_ = false;
    if (cudl_ && cuContext_) {
      cudl_->cuCtxPushCurrent(cuContext_);
      for (int i = 0; i < 2; i++) {
        if (cuResource_[i])
          cudl_->cuGraphicsUnregisterResource(cuResource_[i]);
      }
      cudl_->cuCtxPopCurrent(NULL);
    }
    for (int i = 0; i < 2; i++) {
      textures_[i].Reset();
      SRV_[i].Reset();
    }
    RTV_.Reset();
    vertexShader_.Reset();
    pixelShader_.Reset();
    samplerLinear_.Reset();
  }

  bool prepare() {
    if (prepare_tried_) {
      return prepare_ok_;
    }
    prepare_tried_ = true;

    if (!set_srv())
      return false;
    if (!set_view_port())
      return false;
    if (!set_sample())
      return false;
    if (!set_shader())
      return false;
    if (!set_vertex_buffer())
      return false;
    if (!register_texture())
      return false;

    prepare_ok_ = true;
    return true;
  }

  bool copy_cuda_frame(unsigned char *dpNv12) {
    int width = dec_->GetWidth();
    int height = dec_->GetHeight();
    int chromaHeight = dec_->GetChromaHeight();

    CUVIDAutoCtxPopper ctxPoper(cudl_, cuContext_);

    for (int i = 0; i < 2; i++) {
      CUarray dstArray;
      CUVIDAutoUnmapper unmapper(cudl_, &cuResource_[i]);
      if (!succ(cudl_->cuGraphicsSubResourceGetMappedArray(
              &dstArray, cuResource_[i], 0, 0)))
        return false;
      CUDA_MEMCPY2D m = {0};
      m.srcMemoryType = CU_MEMORYTYPE_DEVICE;
      m.srcDevice = (CUdeviceptr)(dpNv12 + (width * height) * i);
      m.srcPitch = width; // pitch
      m.dstMemoryType = CU_MEMORYTYPE_ARRAY;
      m.dstArray = dstArray;
      m.WidthInBytes = width;
      m.Height = i == 0 ? height : chromaHeight;
      if (!succ(cudl_->cuMemcpy2D(&m)))
        return false;
    }
    return true;
  }

  bool draw() {
    native_->context_->Draw(NUMVERTICES, 0);
    native_->context_->Flush();

    return true;
  }

  // return:
  // >=0: nFrameReturned
  // -1: failed
  // -2: recreated, please decode again
  int decode_and_recreate(uint8_t *data, int len) {
    try {
      int nFrameReturned = dec_->Decode(data, len, CUVID_PKT_ENDOFPICTURE);
      if (nFrameReturned <= 0)
        return -1;
      CUVIDEOFORMAT video_format = dec_->GetLatestVideoFormat();
      auto d1 = last_video_format_.display_area;
      auto d2 = video_format.display_area;
      // reconfigure may cause wrong display area
      if (last_video_format_.coded_width != 0 &&
          (d1.left != d2.left || d1.right != d2.right || d1.top != d2.top ||
           d1.bottom != d2.bottom)) {
        LOG_INFO(
            std::string("recreate, display area changed from (") + std::to_string(d1.left) +
            ", " + std::to_string(d1.top) + ", " + std::to_string(d1.right) +
            ", " + std::to_string(d1.bottom) + ") to (" +
            std::to_string(d2.left) + ", " + std::to_string(d2.top) + ", " +
            std::to_string(d2.right) + ", " + std::to_string(d2.bottom) + ")");
        if (create_nvdecoder()) {
          return -2;
        } else {
          LOG_ERROR(std::string("create_nvdecoder failed"));
        }
        return -1;
      } else {
        return nFrameReturned;
      }
    } catch (const std::exception &e) {
      unsigned int maxWidth = dec_->GetMaxWidth();
      unsigned int maxHeight = dec_->GetMaxHeight();
      CUVIDEOFORMAT video_format = dec_->GetLatestVideoFormat();
      // https://github.com/NVIDIA/DALI/blob/4f5ee72b287cfbbe0d400734416ff37bd8027099/dali/operators/reader/loader/video/frames_decoder_gpu.cc#L212
      if (maxWidth > 0 && (video_format.coded_width > maxWidth ||
                           video_format.coded_height > maxHeight)) {
        LOG_INFO(std::string("recreate, exceed maxWidth/maxHeight: (") +
                 std::to_string(video_format.coded_width) + ", " +
                 std::to_string(video_format.coded_height) + " > (" +
                 std::to_string(maxWidth) + ", " + std::to_string(maxHeight) +
                 ")");
        if (create_nvdecoder()) {
          return -2;
        } else {
          LOG_ERROR(std::string("create_nvdecoder failed"));
        }
      } else {
        LOG_ERROR(std::string("Exception decode_and_recreate: ") + e.what());
      }
    }

    return -1;
  }

  bool set_srv() {
    int width = dec_->GetWidth();
    int height = dec_->GetHeight();
    int chromaHeight = dec_->GetChromaHeight();
    LOG_TRACE(std::string("width:") + std::to_string(width) +
              ", height:" + std::to_string(height) +
              ", chromaHeight:" + std::to_string(chromaHeight));

    D3D11_TEXTURE2D_DESC desc;
    ZeroMemory(&desc, sizeof(desc));
    desc.Width = width;
    desc.Height = height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_R8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.SampleDesc.Quality = 0;
    desc.MiscFlags = 0;
    desc.Usage = D3D11_USAGE_DEFAULT;
    desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    desc.CPUAccessFlags = 0;
    HRB(native_->device_->CreateTexture2D(
        &desc, nullptr, textures_[0].ReleaseAndGetAddressOf()));

    desc.Format = DXGI_FORMAT_R8G8_UNORM;
    desc.Width = width / 2;
    desc.Height = chromaHeight;
    HRB(native_->device_->CreateTexture2D(
        &desc, nullptr, textures_[1].ReleaseAndGetAddressOf()));

    D3D11_SHADER_RESOURCE_VIEW_DESC srvDesc;
    srvDesc = CD3D11_SHADER_RESOURCE_VIEW_DESC(textures_[0].Get(),
                                               D3D11_SRV_DIMENSION_TEXTURE2D,
                                               DXGI_FORMAT_R8_UNORM);
    HRB(native_->device_->CreateShaderResourceView(
        textures_[0].Get(), &srvDesc, SRV_[0].ReleaseAndGetAddressOf()));

    srvDesc = CD3D11_SHADER_RESOURCE_VIEW_DESC(textures_[1].Get(),
                                               D3D11_SRV_DIMENSION_TEXTURE2D,
                                               DXGI_FORMAT_R8G8_UNORM);
    HRB(native_->device_->CreateShaderResourceView(
        textures_[1].Get(), &srvDesc, SRV_[1].ReleaseAndGetAddressOf()));

    // set SRV
    std::array<ID3D11ShaderResourceView *, 2> const textureViews = {
        SRV_[0].Get(), SRV_[1].Get()};
    native_->context_->PSSetShaderResources(0, textureViews.size(),
                                            textureViews.data());
    return true;
  }

  bool set_rtv(ID3D11Texture2D *texture) {
    D3D11_RENDER_TARGET_VIEW_DESC rtDesc;
    rtDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    rtDesc.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2D;
    rtDesc.Texture2D.MipSlice = 0;
    HRB(native_->device_->CreateRenderTargetView(
        texture, &rtDesc, RTV_.ReleaseAndGetAddressOf()));

    const float clearColor[4] = {0.0f, 0.0f, 0.0f, 0.0f}; // clear as black
    native_->context_->ClearRenderTargetView(RTV_.Get(), clearColor);
    native_->context_->OMSetRenderTargets(1, RTV_.GetAddressOf(), NULL);

    return true;
  }

  bool set_view_port() {
    int width = dec_->GetWidth();
    int height = dec_->GetHeight();

    D3D11_VIEWPORT vp;
    vp.Width = (FLOAT)(width);
    vp.Height = (FLOAT)(height);
    vp.MinDepth = 0.0f;
    vp.MaxDepth = 1.0f;
    vp.TopLeftX = 0;
    vp.TopLeftY = 0;
    native_->context_->RSSetViewports(1, &vp);

    return true;
  }

  bool set_sample() {
    D3D11_SAMPLER_DESC sampleDesc = CD3D11_SAMPLER_DESC(CD3D11_DEFAULT());
    HRB(native_->device_->CreateSamplerState(
        &sampleDesc, samplerLinear_.ReleaseAndGetAddressOf()));
    native_->context_->PSSetSamplers(0, 1, samplerLinear_.GetAddressOf());
    return true;
  }

  bool set_shader() {
// https://gist.github.com/RomiTT/9c05d36fe339b899793a3252297a5624
#include "pixel_shader_601.h"
#include "vertex_shader.h"
    native_->device_->CreateVertexShader(
        g_VS, ARRAYSIZE(g_VS), nullptr, vertexShader_.ReleaseAndGetAddressOf());
    native_->device_->CreatePixelShader(g_PS, ARRAYSIZE(g_PS), nullptr,
                                        pixelShader_.ReleaseAndGetAddressOf());

    // set InputLayout
    constexpr std::array<D3D11_INPUT_ELEMENT_DESC, 2> Layout = {{
        {"POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0,
         D3D11_INPUT_PER_VERTEX_DATA, 0},
        {"TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12,
         D3D11_INPUT_PER_VERTEX_DATA, 0},
    }};
    ComPtr<ID3D11InputLayout> inputLayout = NULL;
    HRB(native_->device_->CreateInputLayout(Layout.data(), Layout.size(), g_VS,
                                            ARRAYSIZE(g_VS),
                                            inputLayout.GetAddressOf()));
    native_->context_->IASetInputLayout(inputLayout.Get());

    native_->context_->VSSetShader(vertexShader_.Get(), NULL, 0);
    native_->context_->PSSetShader(pixelShader_.Get(), NULL, 0);

    return true;
  }

  bool set_vertex_buffer() {
    UINT Stride = sizeof(VERTEX);
    UINT Offset = 0;
    FLOAT blendFactor[4] = {0.f, 0.f, 0.f, 0.f};
    native_->context_->OMSetBlendState(nullptr, blendFactor, 0xffffffff);

    native_->context_->IASetPrimitiveTopology(
        D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

    // set VertexBuffers
    VERTEX Vertices[NUMVERTICES] = {
        {XMFLOAT3(-1.0f, -1.0f, 0), XMFLOAT2(0.0f, 1.0f)},
        {XMFLOAT3(-1.0f, 1.0f, 0), XMFLOAT2(0.0f, 0.0f)},
        {XMFLOAT3(1.0f, -1.0f, 0), XMFLOAT2(1.0f, 1.0f)},
        {XMFLOAT3(1.0f, -1.0f, 0), XMFLOAT2(1.0f, 1.0f)},
        {XMFLOAT3(-1.0f, 1.0f, 0), XMFLOAT2(0.0f, 0.0f)},
        {XMFLOAT3(1.0f, 1.0f, 0), XMFLOAT2(1.0f, 0.0f)},
    };
    D3D11_BUFFER_DESC BufferDesc;
    RtlZeroMemory(&BufferDesc, sizeof(BufferDesc));
    BufferDesc.Usage = D3D11_USAGE_DEFAULT;
    BufferDesc.ByteWidth = sizeof(VERTEX) * NUMVERTICES;
    BufferDesc.BindFlags = D3D11_BIND_VERTEX_BUFFER;
    BufferDesc.CPUAccessFlags = 0;
    D3D11_SUBRESOURCE_DATA InitData;
    RtlZeroMemory(&InitData, sizeof(InitData));
    InitData.pSysMem = Vertices;
    ComPtr<ID3D11Buffer> VertexBuffer = nullptr;
    // Create vertex buffer
    HRB(native_->device_->CreateBuffer(&BufferDesc, &InitData, &VertexBuffer));
    native_->context_->IASetVertexBuffers(0, 1, VertexBuffer.GetAddressOf(),
                                          &Stride, &Offset);

    return true;
  }

  bool register_texture() {
    CUVIDAutoCtxPopper ctxPoper(cudl_, cuContext_);

    bool ret = true;
    for (int i = 0; i < 2; i++) {
      if (!succ(cudl_->cuGraphicsD3D11RegisterResource(
              &cuResource_[i], textures_[i].Get(),
              CU_GRAPHICS_REGISTER_FLAGS_NONE))) {
        ret = false;
        break;
      }
      if (!succ(cudl_->cuGraphicsResourceSetMapFlags(
              cuResource_[i], CU_GRAPHICS_REGISTER_FLAGS_WRITE_DISCARD))) {
        ret = false;
        break;
      }
    }

    return ret;
  }

  bool dataFormat_to_cuCodecID(DataFormat dataFormat, cudaVideoCodec &cuda) {
    switch (dataFormat) {
    case H264:
      cuda = cudaVideoCodec_H264;
      break;
    case H265:
      cuda = cudaVideoCodec_HEVC;
      break;
    default:
      return false;
    }
    return true;
  }

  bool create_nvdecoder() {
    LOG_TRACE(std::string("create nvdecoder"));
    bool bUseDeviceFrame = true;
    bool bLowLatency = true;
    bool bDeviceFramePitched = false; // width=pitch
    cudaVideoCodec cudaCodecID;
    if (!dataFormat_to_cuCodecID(dataFormat_, cudaCodecID)) {
      return false;
    }
    if (dec_) {
      delete dec_;
      dec_ = nullptr;
    }
    dec_ = new NvDecoder(cudl_, cvdl_, cuContext_, bUseDeviceFrame, cudaCodecID,
                         bLowLatency, bDeviceFramePitched);
    return true;
  }
};

} // namespace

extern "C" {

int nv_decode_driver_support() {
  try {
    CudaFunctions *cudl = NULL;
    CuvidFunctions *cvdl = NULL;
    load_driver(&cudl, &cvdl);
    free_driver(&cudl, &cvdl);
    return 0;
  } catch (const std::exception &e) {
  }
  return -1;
}

int nv_destroy_decoder(void *decoder) {
  try {
    CuvidDecoder *p = (CuvidDecoder *)decoder;
    if (p) {
      p->destroy();
      delete p;
      p = NULL;
    }
    return 0;
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("destroy failed: ") + e.what());
  }
  return -1;
}

void *nv_new_decoder(void *device, int64_t luid,
                     DataFormat dataFormat) {
  CuvidDecoder *p = NULL;
  try {
    p = new CuvidDecoder(device, luid, dataFormat);
    if (!p) {
      goto _exit;
    }
    if (p->init())
      return p;
  } catch (const std::exception &ex) {
    LOG_ERROR(std::string("destroy failed: ") + ex.what());
    goto _exit;
  }

_exit:
  if (p) {
    p->destroy();
    delete p;
    p = NULL;
  }
  return NULL;
}

int nv_decode(void *decoder, uint8_t *data, int len, DecodeCallback callback,
              void *obj) {
  try {
    CuvidDecoder *p = (CuvidDecoder *)decoder;
    if (p->decode(data, len, callback, obj) == 0 ) {
      return HWCODEC_SUCCESS;
    }
  } catch (const std::exception &e) {
    LOG_ERROR(std::string("decode failed: ") + e.what());
  }
  return HWCODEC_ERR_COMMON;
}

int nv_test_decode(int64_t *outLuids, int32_t *outVendors, int32_t maxDescNum,
                   int32_t *outDescNum, DataFormat dataFormat,
                   uint8_t *data, int32_t length, const int64_t *excludedLuids, const int32_t *excludeFormats, int32_t excludeCount) {
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
      
      CuvidDecoder *p = (CuvidDecoder *)nv_new_decoder(
          nullptr, currentLuid, dataFormat);
      if (!p)
        continue;
      auto start = util::now();
      bool succ = nv_decode(p, data, length, nullptr, nullptr) == 0;
      int64_t elapsed = util::elapsed_ms(start);
      if (succ && elapsed < TEST_TIMEOUT_MS) {
        outLuids[count] = currentLuid;
        outVendors[count] = VENDOR_NV;
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

#include <array>
#include <atomic>
#include <chrono>
#include <cstdio>
#include <list>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include <d3d11.h>
#include <dxgi.h>

#include "win.h"

#define LOG_MODULE "WIN"
#include "log.h"

#define NUMVERTICES 6

typedef struct _VERTEX {
  DirectX::XMFLOAT3 Pos;
  DirectX::XMFLOAT2 TexCoord;
} VERTEX;

bool NativeDevice::Init(int64_t luid, ID3D11Device *device, int pool_size) {
  if (device) {
    if (!InitFromDevice(device))
      return false;
  } else {
    if (!InitFromLuid(luid))
      return false;
  }
  if (!SetMultithreadProtected())
    return false;
  if (!InitQuery())
    return false;
  if (!InitVideoDevice())
    return false;
  count_ = pool_size;
  texture_.resize(count_);
  std::fill(texture_.begin(), texture_.end(), nullptr);
  return true;
}

bool NativeDevice::InitFromLuid(int64_t luid) {
  HRESULT hr = S_OK;

  HRB(CreateDXGIFactory1(IID_IDXGIFactory1,
                         (void **)factory1_.ReleaseAndGetAddressOf()));

  ComPtr<IDXGIAdapter1> tmpAdapter = nullptr;
  UINT i = 0;
  while (!FAILED(
      factory1_->EnumAdapters1(i, tmpAdapter.ReleaseAndGetAddressOf()))) {
    i++;
    DXGI_ADAPTER_DESC1 desc = DXGI_ADAPTER_DESC1();
    tmpAdapter->GetDesc1(&desc);
    if (LUID(desc) == luid) {
      adapter1_.Swap(tmpAdapter);
      break;
    }
  }
  if (!adapter1_) {
    LOG_ERROR(std::string("Failed to find adapter1_"));
    return false;
  }
  HRB(adapter1_.As(&adapter_));

  UINT createDeviceFlags =
      D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT;
  D3D_FEATURE_LEVEL featureLevels[] = {
      D3D_FEATURE_LEVEL_11_0,
  };
  UINT numFeatureLevels = ARRAYSIZE(featureLevels);

  D3D_FEATURE_LEVEL featureLevel;
  D3D_DRIVER_TYPE d3dDriverType =
      adapter1_ ? D3D_DRIVER_TYPE_UNKNOWN : D3D_DRIVER_TYPE_HARDWARE;
  HRB(D3D11CreateDevice(adapter1_.Get(), d3dDriverType, nullptr,
                        createDeviceFlags, featureLevels, numFeatureLevels,
                        D3D11_SDK_VERSION, device_.ReleaseAndGetAddressOf(),
                        &featureLevel, context_.ReleaseAndGetAddressOf()));

  if (featureLevel != D3D_FEATURE_LEVEL_11_0) {
    LOG_ERROR(std::string("Direct3D Feature Level 11 unsupported."));
    return false;
  }
  return true;
}

bool NativeDevice::InitFromDevice(ID3D11Device *device) {
  device_ = device;
  device_->GetImmediateContext(context_.ReleaseAndGetAddressOf());
  ComPtr<IDXGIDevice> dxgiDevice = nullptr;
  HRB(device_.As(&dxgiDevice));
  HRB(dxgiDevice->GetAdapter(adapter_.ReleaseAndGetAddressOf()));
  HRB(adapter_.As(&adapter1_));
  HRB(adapter1_->GetParent(IID_PPV_ARGS(&factory1_)));

  return true;
}

bool NativeDevice::SetMultithreadProtected() {
  ComPtr<ID3D10Multithread> hmt = nullptr;
  HRB(context_.As(&hmt));
  if (!hmt->SetMultithreadProtected(TRUE)) {
    if (!hmt->GetMultithreadProtected()) {
      LOG_ERROR(std::string("Failed to SetMultithreadProtected"));
      return false;
    }
  }
  return true;
}

bool NativeDevice::InitQuery() {
  D3D11_QUERY_DESC queryDesc;
  ZeroMemory(&queryDesc, sizeof(queryDesc));
  queryDesc.Query = D3D11_QUERY_EVENT;
  queryDesc.MiscFlags = 0;
  HRB(device_->CreateQuery(&queryDesc, query_.ReleaseAndGetAddressOf()));
  return true;
}

bool NativeDevice::InitVideoDevice() {
  HRB(device_.As(&video_device_));
  HRB(context_.As(&video_context_));
  HRB(video_context_.As(&video_context1_));
  return true;
}

bool NativeDevice::Nv12ToBgra(int width, int height,
                              ID3D11Texture2D *nv12Texture,
                              ID3D11Texture2D *bgraTexture,
                              int nv12ArrayIndex) {
  if (width != last_nv12_to_bgra_width_ ||
      height != last_nv12_to_bgra_height_) {
    if (!nv12_to_bgra_set_srv(nv12Texture, width, height))
      return false;
    if (!nv12_to_bgra_set_view_port(width, height))
      return false;
    if (!nv12_to_bgra_set_sample())
      return false;
    if (!nv12_to_bgra_set_shader())
      return false;
    if (!nv12_to_bgra_set_vertex_buffer())
      return false;
  }
  last_nv12_to_bgra_width_ = width;
  last_nv12_to_bgra_height_ = height;
  if (!nv12_to_bgra_set_rtv(bgraTexture, width, height))
    return false;

  D3D11_BOX srcBox;
  srcBox.left = 0;
  srcBox.top = 0;
  srcBox.right = width;
  srcBox.bottom = height;
  srcBox.front = 0;
  srcBox.back = 1;
  context_->CopySubresourceRegion(nv12SrvTexture_.Get(), 0, 0, 0, 0,
                                  nv12Texture, nv12ArrayIndex, &srcBox);
  if (!nv12_to_bgra_draw())
    return false;
  return true;
}

bool NativeDevice::nv12_to_bgra_set_srv(ID3D11Texture2D *nv12Texture, int width,
                                        int height) {
  SRV_[0].Reset();
  SRV_[1].Reset();

  D3D11_TEXTURE2D_DESC texDesc = {};
  nv12Texture->GetDesc(&texDesc);
  texDesc.MipLevels = 1;
  texDesc.ArraySize = 1;
  texDesc.Format = DXGI_FORMAT_NV12;
  texDesc.SampleDesc.Quality = 0;
  texDesc.SampleDesc.Count = 1;
  texDesc.Usage = D3D11_USAGE_DEFAULT;
  texDesc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
  texDesc.CPUAccessFlags = 0;
  texDesc.MiscFlags = 0;

  texDesc.Width = width;
  texDesc.Height = height;

  HRB(device_->CreateTexture2D(&texDesc, nullptr,
                               nv12SrvTexture_.ReleaseAndGetAddressOf()));

  D3D11_SHADER_RESOURCE_VIEW_DESC srvDesc;
  srvDesc = CD3D11_SHADER_RESOURCE_VIEW_DESC(nv12SrvTexture_.Get(),
                                             D3D11_SRV_DIMENSION_TEXTURE2D,
                                             DXGI_FORMAT_R8_UNORM);
  HRB(device_->CreateShaderResourceView(nv12SrvTexture_.Get(), &srvDesc,
                                        SRV_[0].ReleaseAndGetAddressOf()));

  srvDesc = CD3D11_SHADER_RESOURCE_VIEW_DESC(nv12SrvTexture_.Get(),
                                             D3D11_SRV_DIMENSION_TEXTURE2D,
                                             DXGI_FORMAT_R8G8_UNORM);
  HRB(device_->CreateShaderResourceView(nv12SrvTexture_.Get(), &srvDesc,
                                        SRV_[1].ReleaseAndGetAddressOf()));

  // set SRV
  std::array<ID3D11ShaderResourceView *, 2> const textureViews = {
      SRV_[0].Get(), SRV_[1].Get()};
  context_->PSSetShaderResources(0, textureViews.size(), textureViews.data());
  return true;
}

bool NativeDevice::nv12_to_bgra_set_rtv(ID3D11Texture2D *bgraTexture, int width,
                                        int height) {
  RTV_.Reset();

  D3D11_RENDER_TARGET_VIEW_DESC rtDesc;
  rtDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  rtDesc.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2D;
  rtDesc.Texture2D.MipSlice = 0;
  HRB(device_->CreateRenderTargetView(bgraTexture, &rtDesc,
                                      RTV_.ReleaseAndGetAddressOf()));

  const float clearColor[4] = {0.0f, 0.0f, 0.0f, 0.0f}; // clear as black
  context_->ClearRenderTargetView(RTV_.Get(), clearColor);
  context_->OMSetRenderTargets(1, RTV_.GetAddressOf(), NULL);

  return true;
}

bool NativeDevice::nv12_to_bgra_set_view_port(int width, int height) {

  D3D11_VIEWPORT vp;
  vp.Width = (FLOAT)(width);
  vp.Height = (FLOAT)(height);
  vp.MinDepth = 0.0f;
  vp.MaxDepth = 1.0f;
  vp.TopLeftX = 0;
  vp.TopLeftY = 0;
  context_->RSSetViewports(1, &vp);

  return true;
}

bool NativeDevice::nv12_to_bgra_set_sample() {
  samplerLinear_.Reset();

  D3D11_SAMPLER_DESC sampleDesc = CD3D11_SAMPLER_DESC(CD3D11_DEFAULT());
  HRB(device_->CreateSamplerState(&sampleDesc,
                                  samplerLinear_.ReleaseAndGetAddressOf()));
  context_->PSSetSamplers(0, 1, samplerLinear_.GetAddressOf());
  return true;
}

bool NativeDevice::nv12_to_bgra_set_shader() {
  vertexShader_.Reset();
  pixelShader_.Reset();

// https://gist.github.com/RomiTT/9c05d36fe339b899793a3252297a5624
#include "pixel_shader_601.h"
#include "vertex_shader.h"
  device_->CreateVertexShader(g_VS, ARRAYSIZE(g_VS), nullptr,
                              vertexShader_.ReleaseAndGetAddressOf());
  device_->CreatePixelShader(g_PS, ARRAYSIZE(g_PS), nullptr,
                             pixelShader_.ReleaseAndGetAddressOf());

  // set InputLayout
  constexpr std::array<D3D11_INPUT_ELEMENT_DESC, 2> Layout = {{
      {"POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0,
       D3D11_INPUT_PER_VERTEX_DATA, 0},
      {"TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12,
       D3D11_INPUT_PER_VERTEX_DATA, 0},
  }};
  ComPtr<ID3D11InputLayout> inputLayout = NULL;
  HRB(device_->CreateInputLayout(Layout.data(), Layout.size(), g_VS,
                                 ARRAYSIZE(g_VS), inputLayout.GetAddressOf()));
  context_->IASetInputLayout(inputLayout.Get());

  context_->VSSetShader(vertexShader_.Get(), NULL, 0);
  context_->PSSetShader(pixelShader_.Get(), NULL, 0);

  return true;
}

bool NativeDevice::nv12_to_bgra_set_vertex_buffer() {
  UINT Stride = sizeof(VERTEX);
  UINT Offset = 0;
  FLOAT blendFactor[4] = {0.f, 0.f, 0.f, 0.f};
  context_->OMSetBlendState(nullptr, blendFactor, 0xffffffff);
  context_->IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
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
  HRB(device_->CreateBuffer(&BufferDesc, &InitData, &VertexBuffer));
  context_->IASetVertexBuffers(0, 1, VertexBuffer.GetAddressOf(), &Stride,
                               &Offset);

  return true;
}

bool NativeDevice::nv12_to_bgra_draw() {
  context_->Draw(NUMVERTICES, 0);
  context_->Flush();
  return true;
}

bool NativeDevice::EnsureTexture(int width, int height) {
  D3D11_TEXTURE2D_DESC desc;
  ZeroMemory(&desc, sizeof(desc));
  if (texture_[0]) {
    texture_[0]->GetDesc(&desc);
    if ((int)desc.Width == width && (int)desc.Height == height &&
        desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM &&
        desc.MiscFlags == D3D11_RESOURCE_MISC_SHARED &&
        desc.Usage == D3D11_USAGE_DEFAULT) {
      return true;
    }
  }
  desc.Width = width;
  desc.Height = height;
  desc.MipLevels = 1;
  desc.ArraySize = 1;
  desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  desc.SampleDesc.Count = 1;
  desc.SampleDesc.Quality = 0;
  desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED;
  desc.Usage = D3D11_USAGE_DEFAULT;
  desc.BindFlags = D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET;
  desc.CPUAccessFlags = 0;

  for (int i = 0; i < texture_.size(); i++) {
    HRB(device_->CreateTexture2D(&desc, nullptr,
                                 texture_[i].ReleaseAndGetAddressOf()));
  }

  return true;
}

bool NativeDevice::SetTexture(ID3D11Texture2D *texture) {
  texture_[index_].Reset();
  texture_[index_] = texture;
  return true;
}

HANDLE NativeDevice::GetSharedHandle() {
  ComPtr<IDXGIResource> resource = nullptr;
  HRP(texture_[index_].As(&resource));
  HANDLE sharedHandle = nullptr;
  HRP(resource->GetSharedHandle(&sharedHandle));
  return sharedHandle;
}

ID3D11Texture2D *NativeDevice::GetCurrentTexture() {
  return texture_[index_].Get();
}

int NativeDevice::next() {
  index_++;
  index_ = index_ % count_;
  return index_;
}

void NativeDevice::BeginQuery() { context_->Begin(query_.Get()); }

void NativeDevice::EndQuery() { context_->End(query_.Get()); }

bool NativeDevice::Query() {
  BOOL bResult = FALSE;
  int attempts = 0;
  while (!bResult) {
    HRESULT hr = context_->GetData(query_.Get(), &bResult, sizeof(BOOL), 0);
    if (SUCCEEDED(hr)) {
      if (bResult) {
        break;
      }
    }
    attempts++;
    if (attempts > 100)
      Sleep(1);
    if (attempts > 1000)
      break;
  }
  return bResult == TRUE;
}

bool NativeDevice::Process(ID3D11Texture2D *in, ID3D11Texture2D *out, int width,
                           int height,
                           D3D11_VIDEO_PROCESSOR_CONTENT_DESC content_desc,
                           DXGI_COLOR_SPACE_TYPE colorSpace_in,
                           DXGI_COLOR_SPACE_TYPE colorSpace_out,
                           int arraySlice) {
  D3D11_TEXTURE2D_DESC inDesc = {0};
  D3D11_TEXTURE2D_DESC outDesc = {0};
  in->GetDesc(&inDesc);
  out->GetDesc(&outDesc);
  if (memcmp(&last_content_desc_, &content_desc, sizeof(content_desc)) != 0) {
    if (video_processor_enumerator_) {
      video_processor_enumerator_.Reset();
    }
    if (video_processor_) {
      video_processor_.Reset();
    }
  }
  memcpy(&last_content_desc_, &content_desc, sizeof(content_desc));

  if (!video_processor_enumerator_ || !video_processor_) {
    HRB(video_device_->CreateVideoProcessorEnumerator(
        &content_desc, video_processor_enumerator_.ReleaseAndGetAddressOf()));
    HRB(video_device_->CreateVideoProcessor(
        video_processor_enumerator_.Get(), 0,
        video_processor_.ReleaseAndGetAddressOf()));
    // This fix too dark or too light, and also make in/out colorspace work
    video_context_->VideoProcessorSetStreamAutoProcessingMode(
        video_processor_.Get(), 0, FALSE);
    video_context_->VideoProcessorSetStreamFrameFormat(
        video_processor_.Get(), 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
  }

  // https://chromium.googlesource.com/chromium/src/media/+/refs/heads/main/gpu/windows/d3d11_video_processor_proxy.cc#138
  // https://chromium.googlesource.com/chromium/src/+/a30440e4cfc7016d4f75a4e108025667e130b78b/media/gpu/windows/dxva_video_decode_accelerator_win.cc

  video_context1_->VideoProcessorSetStreamColorSpace1(video_processor_.Get(), 0,
                                                      colorSpace_in);
  video_context1_->VideoProcessorSetOutputColorSpace1(video_processor_.Get(),
                                                      colorSpace_out);

  RECT rect = {0};
  rect.right = width;
  rect.bottom = height;
  video_context_->VideoProcessorSetStreamSourceRect(video_processor_.Get(), 0,
                                                    true, &rect);
  video_context1_->VideoProcessorSetStreamDestRect(video_processor_.Get(), 0,
                                                   true, &rect);

  D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC InputViewDesc;
  ZeroMemory(&InputViewDesc, sizeof(InputViewDesc));
  InputViewDesc.FourCC = 0;
  InputViewDesc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
  InputViewDesc.Texture2D.MipSlice = 0;
  InputViewDesc.Texture2D.ArraySlice = arraySlice;
  ComPtr<ID3D11VideoProcessorInputView> inputView = nullptr;
  HRB(video_device_->CreateVideoProcessorInputView(
      in, video_processor_enumerator_.Get(), &InputViewDesc,
      inputView.ReleaseAndGetAddressOf()));

  D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC OutputViewDesc;
  ZeroMemory(&OutputViewDesc, sizeof(OutputViewDesc));
  OutputViewDesc.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;
  OutputViewDesc.Texture2D.MipSlice = 0;
  ComPtr<ID3D11VideoProcessorOutputView> outputView = nullptr;
  video_device_->CreateVideoProcessorOutputView(
      out, video_processor_enumerator_.Get(), &OutputViewDesc,
      outputView.ReleaseAndGetAddressOf());

  D3D11_VIDEO_PROCESSOR_STREAM StreamData;
  ZeroMemory(&StreamData, sizeof(StreamData));
  StreamData.Enable = TRUE;
  StreamData.pInputSurface = inputView.Get();
  HRB(video_context_->VideoProcessorBlt(video_processor_.Get(),
                                        outputView.Get(), 0, 1, &StreamData));

  return true;
}

bool NativeDevice::BgraToNv12(ID3D11Texture2D *bgraTexture,
                              ID3D11Texture2D *nv12Texture, int width,
                              int height, DXGI_COLOR_SPACE_TYPE colorSpace_in,
                              DXGI_COLOR_SPACE_TYPE colorSpace_out) {
  D3D11_TEXTURE2D_DESC bgraDesc = {0};
  D3D11_TEXTURE2D_DESC nv12Desc = {0};
  bgraTexture->GetDesc(&bgraDesc);
  nv12Texture->GetDesc(&nv12Desc);
  if (bgraDesc.Width < width || bgraDesc.Height < height) {
    LOG_ERROR(std::string("bgraTexture size is smaller than width and height, ") +
              std::to_string(bgraDesc.Width) + "x" +
              std::to_string(bgraDesc.Height) + " < " + std::to_string(width) +
              "x" + std::to_string(height));
    return false;
  }
  if (nv12Desc.Width < width || nv12Desc.Height < height) {
    LOG_ERROR(std::string("nv12Texture size is smaller than width and height,") +
              std::to_string(nv12Desc.Width) + "x" +
              std::to_string(nv12Desc.Height) + " < " + std::to_string(width) +
              "x" + std::to_string(height));
    return false;
  }

  D3D11_VIDEO_PROCESSOR_CONTENT_DESC contentDesc;
  ZeroMemory(&contentDesc, sizeof(contentDesc));
  contentDesc.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
  contentDesc.InputFrameRate.Numerator = 30;
  contentDesc.InputFrameRate.Denominator = 1;
  // TODO: width height always same with desc.Width and desc.Height in test,
  // need test for decide to use which one
  // https://chromium.googlesource.com/chromium/src/media/+/refs/heads/main/gpu/windows/d3d11_video_processor_proxy.cc#72
  // https://chromium.googlesource.com/chromium/src/media/+/refs/heads/main/gpu/windows/media_foundation_video_encode_accelerator_win.cc#2170
  contentDesc.InputWidth = width;
  contentDesc.InputHeight = height;
  contentDesc.OutputWidth = width;
  contentDesc.OutputHeight = height;
  contentDesc.OutputFrameRate.Numerator = 30;
  contentDesc.OutputFrameRate.Denominator = 1;

  return Process(bgraTexture, nv12Texture, width, height, contentDesc,
                 colorSpace_in, colorSpace_out, 0);
}

AdapterVendor NativeDevice::GetVendor() {
  DXGI_ADAPTER_DESC1 desc1 = DXGI_ADAPTER_DESC1();
  adapter1_->GetDesc1(&desc1);
  if (desc1.VendorId == ADAPTER_VENDOR_NVIDIA) {
    return ADAPTER_VENDOR_NVIDIA;
  } else if (desc1.VendorId == ADAPTER_VENDOR_AMD) {
    return ADAPTER_VENDOR_AMD;
  } else if (desc1.VendorId == ADAPTER_VENDOR_INTEL) {
    return ADAPTER_VENDOR_INTEL;
  } else {
    return ADAPTER_VENDOR_UNKNOWN;
  }
}

bool NativeDevice::support_decode(DataFormat format) {
  const GUID *guid = nullptr;
  switch (format) {
  case H264:
    guid = &D3D11_DECODER_PROFILE_H264_VLD_NOFGT;
    break;
  case H265:
    guid = &D3D11_DECODER_PROFILE_HEVC_VLD_MAIN;
    break;
  default:
    return false;
  }
  BOOL supported = FALSE;
  if (S_OK != video_device_->CheckVideoDecoderFormat(guid, DXGI_FORMAT_NV12,
                                                     &supported)) {
    return false;
  }
  if (supported) {
    DXGI_ADAPTER_DESC1 desc1 = DXGI_ADAPTER_DESC1();
    if (FAILED(adapter1_->GetDesc1(&desc1))) {
      return false;
    }
    bool partial =
        isFormatHybridDecodedByHardware(format, desc1.VendorId, desc1.DeviceId);
    return partial == false;
  }
  return false;
}

// https://github.com/moonlight-stream/moonlight-qt/blob/9117f6565e4b2a6ba5417282de6bf9360b681f1a/app/streaming/video/ffmpeg-renderers/dxutil.h#L8
bool NativeDevice::isFormatHybridDecodedByHardware(DataFormat format,
                                                   unsigned int vendorId,
                                                   unsigned int deviceId) {
  if (vendorId == ADAPTER_VENDOR_INTEL) {
    // Intel seems to encode the series in the high byte of
    // the device ID. We want to avoid the "Partial" acceleration
    // support explicitly. Those will claim to have HW acceleration
    // but perform badly.
    // https://en.wikipedia.org/wiki/Intel_Graphics_Technology#Capabilities_(GPU_video_acceleration)
    // https://raw.githubusercontent.com/GameTechDev/gpudetect/master/IntelGfx.cfg
    switch (deviceId & 0xFF00) {
    case 0x0400: // Haswell
    case 0x0A00: // Haswell
    case 0x0D00: // Haswell
    case 0x1600: // Broadwell
    case 0x2200: // Cherry Trail and Braswell
      // Block these for HEVC to avoid hybrid decode
      return format == H265;
    default:
      break;
    }
  } else if (vendorId == ADAPTER_VENDOR_NVIDIA) {
    // For NVIDIA, we wait to avoid those GPUs with Feature Set E
    // for HEVC decoding, since that's hybrid. It appears that Kepler GPUs
    // also had some hybrid decode support (per DXVA2 Checker) so we'll
    // blacklist those too.
    // https://en.wikipedia.org/wiki/Nvidia_PureVideo
    // https://bluesky23.yukishigure.com/en/dxvac/deviceInfo/decoder.html
    // http://envytools.readthedocs.io/en/latest/hw/pciid.html (missing GM200)
    if ((deviceId >= 0x1180 && deviceId <= 0x11BF) || // GK104
        (deviceId >= 0x11C0 && deviceId <= 0x11FF) || // GK106
        (deviceId >= 0x0FC0 && deviceId <= 0x0FFF) || // GK107
        (deviceId >= 0x1000 && deviceId <= 0x103F) || // GK110/GK110B
        (deviceId >= 0x1280 && deviceId <= 0x12BF) || // GK208
        (deviceId >= 0x1340 && deviceId <= 0x137F) || // GM108
        (deviceId >= 0x1380 && deviceId <= 0x13BF) || // GM107
        (deviceId >= 0x13C0 && deviceId <= 0x13FF) || // GM204
        (deviceId >= 0x1617 && deviceId <= 0x161A) || // GM204
        (deviceId == 0x1667) ||                       // GM204
        (deviceId >= 0x17C0 && deviceId <= 0x17FF)) { // GM200
      // Avoid HEVC on Feature Set E GPUs
      return format == H265;
    }
  }

  return false;
}

bool Adapter::Init(IDXGIAdapter1 *adapter1) {
  HRESULT hr = S_OK;

  adapter1_ = adapter1;
  HRB(adapter1_.As(&adapter_));

  UINT createDeviceFlags = 0;
  D3D_FEATURE_LEVEL featureLevels[] = {
      D3D_FEATURE_LEVEL_11_0,
  };
  UINT numFeatureLevels = ARRAYSIZE(featureLevels);

  D3D_FEATURE_LEVEL featureLevel;
  D3D_DRIVER_TYPE d3dDriverType =
      adapter1_ ? D3D_DRIVER_TYPE_UNKNOWN : D3D_DRIVER_TYPE_HARDWARE;
  hr = D3D11CreateDevice(adapter1_.Get(), d3dDriverType, nullptr,
                         createDeviceFlags, featureLevels, numFeatureLevels,
                         D3D11_SDK_VERSION, device_.ReleaseAndGetAddressOf(),
                         &featureLevel, context_.ReleaseAndGetAddressOf());

  if (FAILED(hr)) {
    return false;
  }

  if (featureLevel != D3D_FEATURE_LEVEL_11_0) {
    std::cerr << "Direct3D Feature Level 11 unsupported." << std::endl;
    return false;
  }

  HRB(adapter1->GetDesc1(&desc1_));
  if (desc1_.VendorId == ADAPTER_VENDOR_INTEL) {
    if (!SetMultithreadProtected())
      return false;
  }

  return true;
}

bool Adapter::SetMultithreadProtected() {
  ComPtr<ID3D10Multithread> hmt = nullptr;
  HRB(context_.As(&hmt));
  if (!hmt->SetMultithreadProtected(TRUE)) {
    if (!hmt->GetMultithreadProtected()) {
      std::cerr << "Failed to SetMultithreadProtected" << std::endl;
      return false;
    }
  }
  return true;
}

bool Adapters::Init(AdapterVendor vendor) {
  HRB(CreateDXGIFactory1(IID_IDXGIFactory1,
                         (void **)factory1_.ReleaseAndGetAddressOf()));

  ComPtr<IDXGIAdapter1> tmpAdapter = nullptr;
  UINT i = 0;
  while (!FAILED(
      factory1_->EnumAdapters1(i, tmpAdapter.ReleaseAndGetAddressOf()))) {
    i++;
    DXGI_ADAPTER_DESC1 desc = DXGI_ADAPTER_DESC1();
    tmpAdapter->GetDesc1(&desc);
    if (desc.VendorId == static_cast<UINT>(vendor)) {
      auto adapter = std::make_unique<Adapter>();
      if (adapter->Init(tmpAdapter.Get())) {
        adapters_.push_back(std::move(adapter));
      }
    }
  }

  return true;
}

int Adapters::GetFirstAdapterIndex(AdapterVendor vendor) {
  ComPtr<IDXGIFactory1> factory1 = nullptr;
  HRI(CreateDXGIFactory1(IID_IDXGIFactory1,
                         (void **)factory1.ReleaseAndGetAddressOf()));

  ComPtr<IDXGIAdapter1> tmpAdapter = nullptr;
  UINT i = 0;
  while (!FAILED(
      factory1->EnumAdapters1(i, tmpAdapter.ReleaseAndGetAddressOf()))) {
    i++;
    DXGI_ADAPTER_DESC1 desc = DXGI_ADAPTER_DESC1();
    tmpAdapter->GetDesc1(&desc);
    if (desc.VendorId == static_cast<UINT>(vendor)) {
      return i - 1;
    }
  }
  return -1;
}

// https://asawicki.info/news_1773_how_to_programmatically_check_graphics_driver_version
// https://github.com/citizenfx/fivem/issues/1121
uint64_t GetHwcodecGpuSignature() {
  uint64_t signature = 0;
  ComPtr<IDXGIFactory1> factory1 = nullptr;
  HRI(CreateDXGIFactory1(IID_IDXGIFactory1,
                         (void **)factory1.ReleaseAndGetAddressOf()));

  ComPtr<IDXGIAdapter1> tmpAdapter = nullptr;
  UINT i = 0;
  while (!FAILED(
      factory1->EnumAdapters1(i, tmpAdapter.ReleaseAndGetAddressOf()))) {
    i++;
    DXGI_ADAPTER_DESC1 desc = {0};
    if (SUCCEEDED(tmpAdapter->GetDesc1(&desc))) {
      if (desc.VendorId == ADAPTER_VENDOR_NVIDIA ||
          desc.VendorId == ADAPTER_VENDOR_AMD ||
          desc.VendorId == ADAPTER_VENDOR_INTEL) {
        // hardware
        signature += desc.VendorId;
        signature += desc.DeviceId;
        signature += desc.SubSysId;
        signature += desc.Revision;
        // software
        LARGE_INTEGER umd_version;
        if SUCCEEDED (tmpAdapter->CheckInterfaceSupport(__uuidof(IDXGIDevice),
                                                        &umd_version)) {
          signature += umd_version.QuadPart;
        }
      }
    }
  }
  return signature;
}

void hwcodec_get_d3d11_texture_width_height(ID3D11Texture2D *texture, int *w,
                                             int *h) {
  D3D11_TEXTURE2D_DESC desc;
  texture->GetDesc(&desc);
  *w = desc.Width;
  *h = desc.Height;
}

int32_t add_process_to_new_job(DWORD process_id) {
  HANDLE job_handle = CreateJobObjectW(nullptr, nullptr);
  if (job_handle == nullptr) {
    LOG_ERROR(std::string("Failed to create job object"));
    return -1;
  }

  JOBOBJECT_EXTENDED_LIMIT_INFORMATION job_info = {0};
  job_info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

  BOOL result = SetInformationJobObject(
      job_handle,
      JobObjectExtendedLimitInformation,
      &job_info,
      sizeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION)
  );

  if (result == FALSE) {
    CloseHandle(job_handle);
    LOG_ERROR(std::string("Failed to set job information"));
    return -1;
  }

  // Open the existing process by ID
  HANDLE process_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, process_id);
  if (process_handle == nullptr) {
    CloseHandle(job_handle);
    LOG_ERROR(std::string("Failed to open process with ID: ") + std::to_string(process_id));
    return -1;
  }

  // Assign the child process to the Job object
  BOOL assign_result = AssignProcessToJobObject(job_handle, process_handle);
  if (assign_result == FALSE) {
    CloseHandle(process_handle);
    CloseHandle(job_handle);
    LOG_ERROR(std::string("Failed to assign process to job"));
    return -1;
  }

  // Close process handle (but keep job handle)
  CloseHandle(process_handle);

  return 0;
}
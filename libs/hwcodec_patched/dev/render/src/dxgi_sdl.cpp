#include <atomic>
#include <chrono>
#include <cstdio>
#include <list>
#include <mutex>
#include <thread>
#include <vector>

#include <DirectXMath.h>
#include <SDL.h>
#include <SDL_syswm.h>
#include <d3d11.h>
#include <d3d11_1.h>
#include <d3d11_2.h>
#include <d3d11_3.h>
#include <d3d11_4.h>
#include <dxgi.h>
#include <dxgi1_2.h>

#include <iostream>
#include <wrl/client.h>

using Microsoft::WRL::ComPtr;

#define SAFE_RELEASE(p)                                                        \
  {                                                                            \
    if ((p)) {                                                                 \
      (p)->Release();                                                          \
      (p) = nullptr;                                                           \
    }                                                                          \
  }
#define LUID(desc)                                                             \
  (((int64_t)desc.AdapterLuid.HighPart << 32) | desc.AdapterLuid.LowPart)
#define HRB(f) MS_CHECK(f, return false;)
#define HRI(f) MS_CHECK(f, return -1;)
#define HRP(f) MS_CHECK(f, return nullptr;)
#define MS_CHECK(f, ...)                                                       \
  do {                                                                         \
    HRESULT __ms_hr__ = (f);                                                   \
    if (FAILED(__ms_hr__)) {                                                   \
      std::clog                                                                \
          << #f "  ERROR@" << __LINE__ << __FUNCTION__ << ": (" << std::hex    \
          << __ms_hr__ << std::dec << ") "                                     \
          << std::error_code(__ms_hr__, std::system_category()).message()      \
          << std::endl                                                         \
          << std::flush;                                                       \
      __VA_ARGS__                                                              \
    }                                                                          \
  } while (false)
#define MS_THROW(f, ...) MS_CHECK(f, throw std::runtime_error(#f);)

#define LUID(desc)                                                             \
  (((int64_t)desc.AdapterLuid.HighPart << 32) | desc.AdapterLuid.LowPart)

#ifndef CSO_DIR
#define CSO_DIR "dev/render/res"
#endif

struct AdatperOutputs {
  IDXGIAdapter1 *adapter;
  DXGI_ADAPTER_DESC1 desc;
  AdatperOutputs() : adapter(nullptr){};
  AdatperOutputs(AdatperOutputs &&src) noexcept {
    adapter = src.adapter;
    src.adapter = nullptr;
    desc = src.desc;
  }
  AdatperOutputs(const AdatperOutputs &src) {
    adapter = src.adapter;
    adapter->AddRef();
    desc = src.desc;
  }
  ~AdatperOutputs() {
    if (adapter)
      adapter->Release();
  }
};

bool get_first_adapter_output(IDXGIFactory2 *factory2,
                              IDXGIAdapter1 **adapter_out,
                              IDXGIOutput1 **output_out, int64_t luid) {
  UINT num_adapters = 0;
  AdatperOutputs curent_adapter;
  IDXGIAdapter1 *selected_adapter = nullptr;
  IDXGIOutput1 *selected_output = nullptr;
  HRESULT hr = S_OK;
  bool found = false;
  while (factory2->EnumAdapters1(num_adapters, &curent_adapter.adapter) !=
         DXGI_ERROR_NOT_FOUND) {
    ++num_adapters;
    DXGI_ADAPTER_DESC1 desc = DXGI_ADAPTER_DESC1();
    curent_adapter.adapter->GetDesc1(&desc);
    if (LUID(desc) != luid) {
      continue;
    }
    selected_adapter = curent_adapter.adapter;
    selected_adapter->AddRef();
    IDXGIOutput *output;
    if (curent_adapter.adapter->EnumOutputs(0, &output) !=
        DXGI_ERROR_NOT_FOUND) {
      IDXGIOutput1 *temp;
      hr = output->QueryInterface(IID_PPV_ARGS(&temp));
      if (SUCCEEDED(hr)) {
        selected_output = temp;
      }
    }
    found = true;
    break;
  }
  *adapter_out = selected_adapter;
  *output_out = selected_output;
  return found;
}

class dx_device_context {
public:
  dx_device_context(int64_t luid) {
    // This is what matters (the 1)
    // Only a guid of fatory2 will not work
    HRESULT hr = CreateDXGIFactory1(IID_PPV_ARGS(&factory2));
    if (FAILED(hr))
      exit(hr);
    if (!get_first_adapter_output(factory2, &adapter1, &output1, luid)) {
      std::cout << "no render adapter found" << std::endl;
      exit(-1);
    }
    D3D_FEATURE_LEVEL levels[]{D3D_FEATURE_LEVEL_11_0};
    hr = D3D11CreateDevice(
        adapter1, D3D_DRIVER_TYPE_UNKNOWN, NULL,
        D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        levels, 1, D3D11_SDK_VERSION, &device, NULL, &context);
    if (FAILED(hr))
      exit(hr);
    hr = device->QueryInterface(IID_PPV_ARGS(&video_device));
    if (FAILED(hr))
      exit(hr);
    hr = context->QueryInterface(IID_PPV_ARGS(&video_context));
    if (FAILED(hr))
      exit(hr);
    hr = context->QueryInterface(IID_PPV_ARGS(&hmt));
    if (FAILED(hr))
      exit(hr);
    // This is required for MFXVideoCORE_SetHandle
    hr = hmt->SetMultithreadProtected(TRUE);
    if (FAILED(hr))
      exit(hr);
  }
  ~dx_device_context() {
    if (hmt)
      hmt->Release();
    if (video_context)
      video_context->Release();
    if (video_device)
      video_device->Release();
    if (context)
      context->Release();
    if (device)
      device->Release();
    if (output1)
      output1->Release();
    if (adapter1)
      adapter1->Release();
    if (factory2)
      factory2->Release();
  }
  IDXGIFactory2 *factory2 = nullptr;
  IDXGIAdapter1 *adapter1 = nullptr;
  IDXGIOutput1 *output1 = nullptr;
  ID3D11Device *device = nullptr;
  ID3D11DeviceContext *context = nullptr;
  ID3D11VideoDevice *video_device = nullptr;
  ID3D11VideoContext *video_context = nullptr;
  ID3D10Multithread *hmt = nullptr;
  HMODULE debug_mod = nullptr;
};

class simplerenderer {
public:
  simplerenderer(HWND in_window, dx_device_context &dev_ctx)
      : ctx(dev_ctx), window(in_window) {
    ctx.factory2->MakeWindowAssociation(in_window, 0);
    sampler_view = nullptr;
    D3D11_SAMPLER_DESC desc = CD3D11_SAMPLER_DESC(CD3D11_DEFAULT());
    HRESULT hr = ctx.device->CreateSamplerState(&desc, &sampler_interp);
    if (FAILED(hr))
      exit(hr);
    init_fbo0(window);
    init_vbo();
    init_shaders();
  }
  ~simplerenderer() {
    // render_thread.join();
    if (sampler_interp)
      sampler_interp->Release();
    if (sampler_view)
      sampler_view->Release();
    if (vbo)
      vbo->Release();
    if (vao)
      vao->Release();
    if (frag)
      frag->Release();
    if (vert)
      vert->Release();
    if (fbo0)
      fbo0->Release();
    if (fbo0rbo)
      fbo0rbo->Release();
    if (swapchain)
      swapchain->Release();
  }

private:
  void init_fbo0(HWND window) {
    DXGI_SWAP_CHAIN_DESC1 swp_desc{
        1280, 720, DXGI_FORMAT_B8G8R8A8_UNORM, FALSE, DXGI_SAMPLE_DESC{1, 0},
        DXGI_USAGE_RENDER_TARGET_OUTPUT, 3, DXGI_SCALING_STRETCH,
        // DXGI_SWAP_EFFECT_DISCARD,
        DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_ALPHA_MODE_UNSPECIFIED, 0};
    HRESULT hr = ctx.factory2->CreateSwapChainForHwnd(
        ctx.device, window, &swp_desc, NULL, NULL, &swapchain);
    if (FAILED(hr))
      exit(hr);
    fbo0 = nullptr;
    fbo0rbo = nullptr;
    RECT rect;
    GetClientRect(window, &rect);
    hr = swapchain->GetBuffer(0, IID_PPV_ARGS(&fbo0rbo));
    if (FAILED(hr))
      exit(hr);
    hr = ctx.device->CreateRenderTargetView(fbo0rbo, nullptr, &fbo0);
    if (FAILED(hr))
      exit(hr);
    D3D11_VIEWPORT VP{};
    VP.Width = static_cast<FLOAT>(rect.right - rect.left);
    VP.Height = static_cast<FLOAT>(rect.bottom - rect.top);
    VP.MinDepth = 0.0f;
    VP.MaxDepth = 1.0f;
    VP.TopLeftX = 0;
    VP.TopLeftY = 0;
    ctx.context->RSSetViewports(1, &VP);
  }
  void init_vbo() {
    struct vertex {
      DirectX::XMFLOAT3 Pos;
      DirectX::XMFLOAT2 TexCoord;
    };
    vertex points[4]{{DirectX::XMFLOAT3(-1, 1, 0), DirectX::XMFLOAT2(0, 0)},
                     {DirectX::XMFLOAT3(1, 1, 0), DirectX::XMFLOAT2(1, 0)},
                     {DirectX::XMFLOAT3(-1, -1, 0), DirectX::XMFLOAT2(0, 1)},
                     {DirectX::XMFLOAT3(1, -1, 0), DirectX::XMFLOAT2(1, 1)}};
    D3D11_BUFFER_DESC vbo_desc{};
    vbo_desc.ByteWidth = sizeof(vertex) * 4;
    vbo_desc.Usage = D3D11_USAGE_IMMUTABLE;
    vbo_desc.BindFlags = D3D11_BIND_VERTEX_BUFFER;
    D3D11_SUBRESOURCE_DATA initial_data{};
    initial_data.pSysMem = points;
    HRESULT hr = ctx.device->CreateBuffer(&vbo_desc, &initial_data, &vbo);
    if (FAILED(hr))
      exit(hr);
    vbo_stride = sizeof(vertex);
    vbo_offset = 0;
  }
  void init_shaders() {
    uint8_t *shader_bytecode = nullptr;
    size_t bytecode_len = 0;
    // read file
    FILE *shader_file = fopen(CSO_DIR "/frag.cso", "rb");
    fseek(shader_file, 0, SEEK_END);
    bytecode_len = ftell(shader_file);
    fseek(shader_file, 0, SEEK_SET);
    shader_bytecode = (uint8_t *)malloc(bytecode_len);
    fread(shader_bytecode, 1, bytecode_len, shader_file);
    HRESULT hr = ctx.device->CreatePixelShader(shader_bytecode, bytecode_len,
                                               nullptr, &frag);
    if (FAILED(hr))
      exit(hr);
    // free(shader_bytecode);
    shader_file = freopen(CSO_DIR "/vert.cso", "rb", shader_file);
    fseek(shader_file, 0, SEEK_END);
    bytecode_len = ftell(shader_file);
    fseek(shader_file, 0, SEEK_SET);
    shader_bytecode = (uint8_t *)malloc(bytecode_len);
    fread(shader_bytecode, 1, bytecode_len, shader_file);
    // fclose(shader_file);
    hr = ctx.device->CreateVertexShader(shader_bytecode, bytecode_len, nullptr,
                                        &vert);
    if (FAILED(hr))
      exit(hr);
    D3D11_INPUT_ELEMENT_DESC input_desc[]{
        // name, vertex attrib index, format, unpack alignment, instance
        // releated
        {"POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0,
         D3D11_INPUT_PER_VERTEX_DATA, 0},
        {"TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12,
         D3D11_INPUT_PER_VERTEX_DATA, 0},
    };
    hr = ctx.device->CreateInputLayout(input_desc, 2, shader_bytecode,
                                       bytecode_len, &vao);
    if (FAILED(hr))
      exit(hr);
    // free(shader_bytecode);
    ctx.context->VSSetShader(vert, nullptr, 0);
    ctx.context->IASetInputLayout(vao);
    ctx.context->IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
    ctx.context->IASetVertexBuffers(0, 1, &vbo, &vbo_stride, &vbo_offset);
    ctx.context->PSSetShader(frag, nullptr, 0);
    ctx.context->PSSetSamplers(0, 1, &sampler_interp);
  }

  void bind_texture(ID3D11Texture2D *texture) {
    D3D11_TEXTURE2D_DESC desc;
    texture->GetDesc(&desc);
    D3D11_SHADER_RESOURCE_VIEW_DESC shader_resource_desc{};
    shader_resource_desc.Format = desc.Format;
    ;
    shader_resource_desc.ViewDimension = D3D11_SRV_DIMENSION_TEXTURE2D;
    shader_resource_desc.Texture2D = {0, 1};
    if (sampler_view)
      sampler_view->Release();
    HRESULT hr = ctx.device->CreateShaderResourceView(
        texture, &shader_resource_desc, &sampler_view);
    if (FAILED(hr))
      exit(hr);
    ctx.context->PSSetShaderResources(0, 1, &sampler_view);
  }
  void resize_swapchain(uint32_t width, uint32_t height) {
    if (fbo0)
      fbo0->Release();
    if (fbo0rbo)
      fbo0rbo->Release();
    HRESULT hr =
        swapchain->ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
    hr = swapchain->GetBuffer(0, IID_PPV_ARGS(&fbo0rbo));
    if (FAILED(hr))
      exit(hr);
    hr = ctx.device->CreateRenderTargetView(fbo0rbo, nullptr, &fbo0);
    if (FAILED(hr))
      exit(hr);
    D3D11_VIEWPORT VP{};
    VP.Width = static_cast<FLOAT>(width);
    VP.Height = static_cast<FLOAT>(height);
    VP.MinDepth = 0.0f;
    VP.MaxDepth = 1.0f;
    VP.TopLeftX = 0;
    VP.TopLeftY = 0;
    ctx.context->RSSetViewports(1, &VP);
  }
  std::chrono::high_resolution_clock::time_point last_fps_time =
      std::chrono::high_resolution_clock::now();

public:
  void render_frame(ID3D11Texture2D *texture) {
    if (!occluded) {
      bind_texture(texture);
      if (need_resize.load(std::memory_order_acquire)) {
        atomic_packed_32x2 temp;
        temp.packed.store(client_size.packed.load(std::memory_order_relaxed),
                          std::memory_order_relaxed);
        resize_swapchain(temp.separate.width, temp.separate.height);
      }
      ctx.context->OMSetRenderTargets(1, &fbo0, nullptr);
      ctx.context->Draw(4, 0);
      HRESULT hr = swapchain->Present(0, 0);
      if (FAILED(hr))
        exit(hr);
      if (hr == DXGI_STATUS_OCCLUDED) {
        occluded = true;
      }
      frame_count++;
      std::chrono::high_resolution_clock::time_point current =
          std::chrono::high_resolution_clock::now();
      if (current - last_fps_time >= std::chrono::seconds(1)) {
        int fps = frame_count - last_frame_count;
        last_frame_count = frame_count;
        last_fps_time = current;
        std::cout << fps << " Hz" << std::endl;
      }
    } else {
      HRESULT hr = swapchain->Present(0, DXGI_PRESENT_TEST);
      if (FAILED(hr))
        exit(hr);
      if (!DXGI_STATUS_OCCLUDED) {
        occluded = false;
      }
    }
  }
  void set_size(uint32_t width, uint32_t height) {
    atomic_packed_32x2 temp;
    temp.separate.width = width;
    temp.separate.height = height;
    client_size.packed.store(temp.packed.load(std::memory_order_relaxed),
                             std::memory_order_relaxed);
  }
  HWND window;
  dx_device_context &ctx;
  IDXGISwapChain1 *swapchain;
  ID3D11Texture2D *fbo0rbo;
  ID3D11RenderTargetView *fbo0;
  ID3D11VertexShader *vert;
  ID3D11PixelShader *frag;
  ID3D11InputLayout *vao;
  ID3D11Buffer *vbo;
  ID3D11ShaderResourceView *sampler_view;
  ID3D11SamplerState *sampler_interp;
  UINT vbo_stride;
  UINT vbo_offset;
  std::thread render_thread;
  std::atomic_bool running;
  std::atomic_bool need_resize;
  bool occluded = false;
  int frame_count = 0;
  int last_frame_count = 0;
  struct atomic_packed_32x2 {
    union {
      struct detail {
        uint32_t width;
        uint32_t height;
      } separate;
      std::atomic_uint64_t packed;
    };
    atomic_packed_32x2();
  } client_size;
};

simplerenderer::atomic_packed_32x2::atomic_packed_32x2(void) {}

class Render {
public:
  Render(int64_t luid, bool inputSharedHandle);
  int Init();
  int RenderTexture(ID3D11Texture2D *);
  std::unique_ptr<std::thread> message_thread;
  std::unique_ptr<simplerenderer> renderer;
  bool running = false;
  std::unique_ptr<dx_device_context> ctx;
  // dx_device_context ctx;
  int64_t luid;
  bool inputSharedHandle;
};

Render::Render(int64_t luid, bool inputSharedHandle) {
  // ctx.reset(new dx_device_context());
  this->luid = luid;
  this->inputSharedHandle = inputSharedHandle;
  ctx = std::make_unique<dx_device_context>(luid);
};

static void run(Render *self) {
  SetProcessDPIAware();
  SDL_Init(SDL_INIT_VIDEO);
  SDL_Window *window = SDL_CreateWindow(
      "test window", SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED, 1280,
      720, SDL_WINDOW_RESIZABLE | SDL_WINDOW_ALLOW_HIGHDPI);
  SDL_SysWMinfo info{};
  SDL_GetWindowWMInfo(window, &info);
  {
    self->renderer.reset(new simplerenderer(info.info.win.window, *self->ctx));
    MONITORINFOEX monitor_info{};
    monitor_info.cbSize = sizeof(monitor_info);
    DXGI_OUTPUT_DESC screen_desc;
    if (self->ctx->output1) {
      HRESULT hr = self->ctx->output1->GetDesc(&screen_desc);
      GetMonitorInfo(screen_desc.Monitor, &monitor_info);
    }
    self->running = true;
    bool maximized = false;
    while (self->running) {
      SDL_Event event;
      SDL_WaitEvent(&event);
      switch (event.type) {
      case SDL_WINDOWEVENT:
        switch (event.window.event) {
        case SDL_WINDOWEVENT_CLOSE:
          // capturer.Stop();
          self->running = false;
          break;
        case SDL_WINDOWEVENT_MAXIMIZED:
          if (self->ctx->output1) {
            int border_l, border_r, border_t, border_b;
            SDL_GetWindowBordersSize(window, &border_t, &border_l, &border_b,
                                     &border_r);
            int max_w = monitor_info.rcWork.right - monitor_info.rcWork.left;
            int max_h =
                monitor_info.rcWork.bottom - monitor_info.rcWork.top - border_t;
            SDL_SetWindowSize(window, max_w, max_h);
            SDL_SetWindowPosition(window, monitor_info.rcWork.left, border_t);
            maximized = true;
          }
          break;
        case SDL_WINDOWEVENT_RESTORED:
          maximized = false;
          break;
        case SDL_WINDOWEVENT_RESIZED:
          if (self->ctx->output1) {
            int max_w, max_h;
            SDL_GetWindowMaximumSize(window, &max_w, &max_h);
            double aspect = double(screen_desc.DesktopCoordinates.right -
                                   screen_desc.DesktopCoordinates.left) /
                            (screen_desc.DesktopCoordinates.bottom -
                             screen_desc.DesktopCoordinates.top);
            int temp = event.window.data1 * event.window.data2;
            int width = sqrt(temp * aspect) + 0.5;
            int height = sqrt(temp / aspect) + 0.5;
            int pos_x, pos_y;
            SDL_GetWindowPosition(window, &pos_x, &pos_y);
            int ori_w, ori_h;
            SDL_GetWindowSize(window, &ori_w, &ori_h);
            SDL_SetWindowPosition(window, pos_x + ((ori_w - width) / 2),
                                  pos_y + ((ori_h - height) / 2));
            SDL_SetWindowSize(window, width, height);
          }
          break;
        case SDL_WINDOWEVENT_SIZE_CHANGED:
          self->renderer->set_size(event.window.data1, event.window.data2);
          break;
        default:
          break;
        }
        break;
      default:
        break;
      }
      if (event.type == SDL_WINDOWEVENT) {
      }
    }
  }

  SDL_DestroyWindow(window);
  SDL_Quit();
  exit(0);
}

int Render::Init() {
  message_thread.reset(new std::thread(run, this));
  return 0;
}

int Render::RenderTexture(ID3D11Texture2D *texture) {
  renderer->render_frame(texture);
  return 0;
}

extern "C" void *CreateDXGIRender(int64_t luid, bool inputSharedHandle) {
  Render *p = new Render(luid, inputSharedHandle);
  p->Init();
  return p;
}

extern "C" int DXGIRenderTexture(void *render, HANDLE handle) {
  Render *self = (Render *)render;
  if (!self->running)
    return 0;
  ComPtr<ID3D11Texture2D> texture = nullptr;
  if (self->inputSharedHandle) {
    ComPtr<IDXGIResource> resource = nullptr;
    ComPtr<ID3D11Texture2D> tex_ = nullptr;
    MS_THROW(self->ctx->device->OpenSharedResource(
        handle, __uuidof(ID3D11Texture2D),
        (void **)resource.ReleaseAndGetAddressOf()));
    MS_THROW(resource.As(&tex_));
    texture = tex_.Get();
  } else {
    texture = (ID3D11Texture2D *)handle;
  }
  self->RenderTexture(texture.Get());

  return 0;
}

extern "C" void DestroyDXGIRender(void *render) {
  Render *self = (Render *)render;
  self->running = false;
  if (self->message_thread)
    self->message_thread->join();
}

extern "C" void *DXGIDevice(void *render) {
  Render *self = (Render *)render;
  return self->ctx->device;
}

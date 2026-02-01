#include <Windows.h>
#include <callback.h>
#include <common.h>
#include <iostream>
#include <stdint.h>
#include <system.h>

extern "C" {
void *dxgi_new_capturer(int64_t luid);
void *dxgi_device(void *self);
int dxgi_width(const void *self);
int dxgi_height(const void *self);
void *dxgi_capture(void *self, int wait_ms);
void destroy_dxgi_capturer(void *self);

void *vpl_new_encoder(void *hdl, int64_t luid, API api, DataFormat dataFormat,
                      int32_t width, int32_t height, int32_t kbs,
                      int32_t framerate, int32_t gop);
int vpl_encode(void *e, void *tex, EncodeCallback callback, void *obj);
int vpl_destroy_encoder(void *e);

void *vpl_new_decoder(void *device, int64_t luid, int32_t api,
                      int32_t dataFormat, bool outputSharedHandle);
int vpl_decode(void *decoder, uint8_t *data, int32_t length,
               DecodeCallback callback, void *obj);
int vpl_destroy_decoder(void *decoder);

void *CreateDXGIRender(long long luid, bool inputSharedHandle);
int DXGIRenderTexture(void *render, HANDLE shared_handle);
void DestroyDXGIRender(void *render);
void *DXGIDevice(void *render);
}

static const uint8_t *encode_data;
static int32_t encode_len;
static void *decode_shared_handle;

extern "C" static void encode_callback(const uint8_t *data, int32_t len,
                                       int32_t key, const void *obj) {
  encode_data = data;
  encode_len = len;
}

extern "C" static void decode_callback(void *shared_handle, const void *obj) {
  decode_shared_handle = shared_handle;
}

extern "C" void log_gpucodec(int level, const char *message) {
  std::cout << message << std::endl;
}
int main() {
  Adapters adapters;
  adapters.Init(ADAPTER_VENDOR_INTEL);
  if (adapters.adapters_.size() == 0) {
    std::cout << "no intel adapter" << std::endl;
    return -1;
  }
  int64_t luid = LUID(adapters.adapters_[0].get()->desc1_);
  DataFormat dataFormat = H264;
  void *dup = dxgi_new_capturer(luid);
  if (!dup) {
    std::cerr << "create duplicator failed" << std::endl;
    return -1;
  }
  int width = dxgi_width(dup);
  int height = dxgi_height(dup);
  std::cout << "width: " << width << " height: " << height << std::endl;
  void *device = dxgi_device(dup);
  void *encoder = vpl_new_encoder(device, luid, API_DX11, dataFormat, width,
                                  height, 4000, 30, 0xFFFF);
  if (!encoder) {
    std::cerr << "create encoder failed" << std::endl;
    return -1;
  }

  void *render = CreateDXGIRender(luid, false);
  if (!render) {
    std::cerr << "create render failed" << std::endl;
    return -1;
  }

  void *decoder =
      vpl_new_decoder(DXGIDevice(render), luid, API_DX11, dataFormat, false);
  if (!decoder) {
    std::cerr << "create decoder failed" << std::endl;
    return -1;
  }

  while (true) {
    void *texture = dxgi_capture(dup, 100);
    if (!texture) {
      std::cerr << "texture is NULL" << std::endl;
      continue;
    }
    if (0 != vpl_encode(encoder, texture, encode_callback, NULL)) {
      std::cerr << "encode failed" << std::endl;
      continue;
    }
    if (0 != vpl_decode(decoder, (uint8_t *)encode_data, encode_len,
                        decode_callback, NULL)) {
      std::cerr << "decode failed" << std::endl;
      continue;
    }
    if (0 != DXGIRenderTexture(render, decode_shared_handle)) {
      std::cerr << "render failed" << std::endl;
      continue;
    }
  }

  // no release temporarily
}
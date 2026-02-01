#include <DDA.h>
#include <Windows.h>
#include <string>

extern "C" void *dxgi_new_capturer(int64_t luid) {
  DemoApplication *d = new DemoApplication(luid);
  HRESULT hr = d->Init();
  if (FAILED(hr)) {
    delete d;
    d = NULL;
    return NULL;
  }

  return d;
}

extern "C" void *dxgi_device(void *capturer) {
  DemoApplication *d = (DemoApplication *)capturer;
  return d->Device();
}

extern "C" int dxgi_width(const void *capturer) {
  DemoApplication *d = (DemoApplication *)capturer;
  return d->width();
}

extern "C" int dxgi_height(const void *capturer) {
  DemoApplication *d = (DemoApplication *)capturer;
  return d->height();
}

extern "C" void *dxgi_capture(void *capturer, int wait_ms) {
  DemoApplication *d = (DemoApplication *)capturer;
  void *texture = d->Capture(wait_ms);
  return texture;
}

extern "C" void destroy_dxgi_capturer(void *capturer) {
  DemoApplication *d = (DemoApplication *)capturer;
  if (d)
    delete d;
}
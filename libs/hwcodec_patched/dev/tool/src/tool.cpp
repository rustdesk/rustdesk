#include <memory.h>

#include "common.h"
#include "system.h"

namespace {

class Tool {
public:
  std::unique_ptr<NativeDevice> native_;
  bool initialized_ = false;

public:
  Tool(int64_t luid) {
    native_ = std::make_unique<NativeDevice>();
    initialized_ = native_->Init(luid, nullptr, 1);
  }

  ID3D11Texture2D *GetTexture(int width, int height) {
    native_->EnsureTexture(width, height);
    return native_->GetCurrentTexture();
  }

  void getSize(ID3D11Texture2D *texture, int *width, int *height) {
    D3D11_TEXTURE2D_DESC desc;
    texture->GetDesc(&desc);
    *width = desc.Width;
    *height = desc.Height;
  }
};
} // namespace

extern "C" {

void *tool_new(int64_t luid) {
  Tool *t = new Tool(luid);
  if (t && !t->initialized_) {
    delete t;
    return nullptr;
  }
  return t;
}

void *tool_device(void *tool) {
  Tool *t = (Tool *)tool;
  return t->native_->device_.Get();
}

void *tool_get_texture(void *tool, int width, int height) {
  Tool *t = (Tool *)tool;
  return t->GetTexture(width, height);
}

void tool_get_texture_size(void *tool, void *texture, int *width, int *height) {
  Tool *t = (Tool *)tool;
  t->getSize((ID3D11Texture2D *)texture, width, height);
}

void tool_destroy(void *tool) {
  Tool *t = (Tool *)tool;
  if (t) {
    delete t;
    t = nullptr;
  }
}

} // extern "C"
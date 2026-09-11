#ifndef RUSTDESK_CURSOR_SIZE_H_
#define RUSTDESK_CURSOR_SIZE_H_

#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>

#include <algorithm>
#include <cstdint>
#include <stdexcept>

namespace cursor_size {

struct IconBitmaps {
  ICONINFO info{};
  ~IconBitmaps() {
    if (info.hbmColor) DeleteObject(info.hbmColor);
    if (info.hbmMask) DeleteObject(info.hbmMask);
  }
};

struct Surface {
  HDC dc = CreateCompatibleDC(nullptr);
  HBITMAP bitmap = nullptr;
  HGDIOBJ previous = nullptr;
  uint32_t* pixels = nullptr;
  int width = 0, height = 0;

  ~Surface() {
    if (previous) SelectObject(dc, previous);
    if (bitmap) DeleteObject(bitmap);
    if (dc) DeleteDC(dc);
  }

  void Init(const BITMAP& source, bool monochrome) {
    width = source.bmWidth;
    height = monochrome ? source.bmHeight / 2 : source.bmHeight;
    if (!dc || width <= 0 || height <= 0) {
      throw std::runtime_error("Could not create the cursor measurement surface");
    }
    constexpr WORD kColorBits = 32;
    BITMAPINFO format{};
    format.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    format.bmiHeader.biWidth = width;
    format.bmiHeader.biHeight = -height;
    format.bmiHeader.biPlanes = 1;
    format.bmiHeader.biBitCount = kColorBits;
    format.bmiHeader.biCompression = BI_RGB;
    bitmap = CreateDIBSection(dc, &format, DIB_RGB_COLORS,
                             reinterpret_cast<void**>(&pixels), nullptr, 0);
    if (!bitmap) throw std::runtime_error("Could not allocate the cursor bitmap");
    previous = SelectObject(dc, bitmap);
    if (!previous || previous == HGDI_ERROR) {
      previous = nullptr;
      throw std::runtime_error("Could not select the cursor bitmap");
    }
  }
};

inline void IncludeVisiblePixels(const Surface& surface, uint32_t background,
                                 RECT& bounds) {
  constexpr uint32_t kRgbMask = 0x00ffffff;
  for (int y = 0; y < surface.height; ++y) {
    for (int x = 0; x < surface.width; ++x) {
      if ((surface.pixels[y * surface.width + x] & kRgbMask) == background) continue;
      bounds.left = (std::min)(bounds.left, static_cast<LONG>(x));
      bounds.right = (std::max)(bounds.right, static_cast<LONG>(x + 1));
      bounds.top = (std::min)(bounds.top, static_cast<LONG>(y));
      bounds.bottom = (std::max)(bounds.bottom, static_cast<LONG>(y + 1));
    }
  }
}

inline double SystemSize() {
  HCURSOR cursor = LoadCursorW(nullptr, IDC_ARROW);
  IconBitmaps bitmaps;
  if (!cursor || !GetIconInfo(cursor, &bitmaps.info)) {
    throw std::runtime_error("Could not read the Windows system cursor");
  }
  BITMAP source{};
  const auto bitmap = bitmaps.info.hbmColor ? bitmaps.info.hbmColor : bitmaps.info.hbmMask;
  if (!GetObject(bitmap, sizeof(source), &source)) {
    throw std::runtime_error("Could not read the system cursor bitmap size");
  }
  Surface surface;
  surface.Init(source, !bitmaps.info.hbmColor);
  RECT bounds{surface.width, surface.height, 0, 0};
  // Drawing on both backgrounds measures alpha and legacy AND/XOR cursors.
  constexpr uint32_t kBlack = 0x00000000, kWhite = 0x00ffffff;
  for (const auto background : {kBlack, kWhite}) {
    std::fill_n(surface.pixels, surface.width * surface.height, background);
    if (!DrawIconEx(surface.dc, 0, 0, cursor, 0, 0, 0, nullptr, DI_NORMAL) || !GdiFlush()) {
      throw std::runtime_error("Could not draw the Windows system cursor");
    }
    IncludeVisiblePixels(surface, background, bounds);
  }
  if (bounds.right <= bounds.left) {
    throw std::runtime_error("System cursor has no visible pixels");
  }
  return (std::max)(bounds.right - bounds.left, bounds.bottom - bounds.top);
}

inline void Register(flutter::BinaryMessenger* messenger) {
  flutter::MethodChannel<> channel(messenger, "org.rustdesk.rustdesk/cursor",
                                  &flutter::StandardMethodCodec::GetInstance());
  channel.SetMethodCallHandler([](const flutter::MethodCall<>& call,
                                  std::unique_ptr<flutter::MethodResult<>> result) {
    if (call.method_name() != "getSystemCursorSize") {
      result->NotImplemented();
      return;
    }
    try {
      result->Success(flutter::EncodableValue(SystemSize()));
    } catch (const std::exception& error) {
      result->Error("cursor_size", error.what());
    }
  });
}

}  // namespace cursor_size

#endif  // RUSTDESK_CURSOR_SIZE_H_

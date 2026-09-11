#ifndef RUSTDESK_CURSOR_SIZE_H_
#define RUSTDESK_CURSOR_SIZE_H_

#include <flutter_linux/flutter_linux.h>
#include <X11/Xcursor/Xcursor.h>
#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif

#include <algorithm>
#include <memory>
#include <stdexcept>

namespace cursor_size {

template <typename IsVisible>
double VisibleSize(int width, int height, IsVisible visible) {
  int left = width, top = height, right = -1, bottom = -1;
  for (int y = 0; y < height; ++y) {
    for (int x = 0; x < width; ++x) {
      if (!visible(x, y)) continue;
      left = std::min(left, x);
      right = std::max(right, x);
      top = std::min(top, y);
      bottom = std::max(bottom, y);
    }
  }
  if (right < left) throw std::runtime_error("System cursor has no visible pixels");
  return std::max(right - left + 1, bottom - top + 1);
}

inline double WaylandSize(GtkWidget* view) {
  g_autofree gchar* theme = nullptr;
  gint size = 0;
  g_object_get(gtk_widget_get_settings(view), "gtk-cursor-theme-name", &theme,
               "gtk-cursor-theme-size", &size, nullptr);
  // GTK uses 24 when the theme size is unset, and loads at the window scale.
  constexpr int kDefaultThemeSize = 24;
  if (size == 0) size = kDefaultThemeSize;
  const int scale = gtk_widget_get_scale_factor(view);
  if (size < 0 || scale <= 0 || size > G_MAXINT / scale) {
    throw std::runtime_error("Invalid GTK cursor theme size or window scale");
  }
  auto* loaded = XcursorLibraryLoadImage("default", theme, size * scale);
  // Match GTK's CSS default -> traditional left_ptr name mapping.
  if (!loaded) loaded = XcursorLibraryLoadImage("left_ptr", theme, size * scale);
  std::unique_ptr<XcursorImage, decltype(&XcursorImageDestroy)> image(
      loaded, XcursorImageDestroy);
  if (!image) throw std::runtime_error("Could not load the GTK system cursor");
  // GTK reduces the buffer scale until both theme dimensions are divisible.
  // For example, a 64px theme at 3x is displayed using a buffer scale of 2.
  int cursor_scale = scale;
  while (image->width % cursor_scale != 0 || image->height % cursor_scale != 0) {
    --cursor_scale;
  }
  constexpr XcursorPixel kAlphaMask = 0xff000000;
  return VisibleSize(image->width, image->height, [&](int x, int y) {
    return (image->pixels[y * image->width + x] & kAlphaMask) != 0;
  }) / cursor_scale;
}

inline double SystemSize(GtkWidget* view) {
  GdkDisplay* display = gtk_widget_get_display(view);
#ifdef GDK_WINDOWING_WAYLAND
  if (GDK_IS_WAYLAND_DISPLAY(display)) return WaylandSize(view);
#endif
  g_autoptr(GdkCursor) cursor = gdk_cursor_new_from_name(display, "default");
  if (!cursor) throw std::runtime_error("Could not load the GDK system cursor");
  g_autoptr(GdkPixbuf) image = gdk_cursor_get_image(cursor);
  if (!image) throw std::runtime_error("Could not read the GDK system cursor");
  const auto* pixels = gdk_pixbuf_read_pixels(image);
  const int stride = gdk_pixbuf_get_rowstride(image);
  const int channels = gdk_pixbuf_get_n_channels(image);
  return VisibleSize(gdk_pixbuf_get_width(image), gdk_pixbuf_get_height(image),
                    [&](int x, int y) {
    return !gdk_pixbuf_get_has_alpha(image) ||
           pixels[y * stride + x * channels + channels - 1] != 0;
  });
}

inline void Register(FlView* view) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  FlMethodChannel* channel = fl_method_channel_new(
      fl_engine_get_binary_messenger(fl_view_get_engine(view)),
      "org.rustdesk.rustdesk/cursor", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel,
      [](FlMethodChannel*, FlMethodCall* call, gpointer data) {
    g_autoptr(GError) error = nullptr;
    if (g_strcmp0(fl_method_call_get_name(call), "getSystemCursorSize") != 0) {
      fl_method_call_respond_not_implemented(call, &error);
    } else {
      try {
        g_autoptr(FlValue) value = fl_value_new_float(SystemSize(GTK_WIDGET(data)));
        fl_method_call_respond_success(call, value, &error);
      } catch (const std::exception& failure) {
        fl_method_call_respond_error(call, "cursor_size", failure.what(), nullptr, &error);
      }
    }
    if (error) g_warning("Cursor size response failed: %s", error->message);
  }, view, nullptr);
  g_object_set_data_full(G_OBJECT(view), "cursor-size-channel", channel, g_object_unref);
}

}  // namespace cursor_size

#endif  // RUSTDESK_CURSOR_SIZE_H_

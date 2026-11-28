#include "official_window_controls.h"

#include <gtk/gtk.h>

#include <limits>

namespace {
FlMethodResponse* reply_ok(FlValue* value = nullptr) {
  return FL_METHOD_RESPONSE(fl_method_success_response_new(value));
}

FlMethodResponse* reply_error(const char* code, const char* message) {
  return FL_METHOD_RESPONSE(fl_method_error_response_new(code, message, nullptr));
}

bool read_int(FlValue* args, const char* key, int64_t* value) {
  FlValue* entry = fl_value_lookup_string(args, key);
  if (!entry || fl_value_get_type(entry) != FL_VALUE_TYPE_INT) return false;
  *value = fl_value_get_int(entry);
  return true;
}

GtkWindow* lookup_window(FlValue* args) {
  int64_t handle = 0;
  if (!args || fl_value_get_type(args) != FL_VALUE_TYPE_MAP ||
      !read_int(args, "handle", &handle) || handle == 0) return nullptr;
  // Validate against GTK's live windows without dereferencing a stale handle.
  GList* windows = gtk_window_list_toplevels();
  GList* entry = g_list_find(windows, GSIZE_TO_POINTER(handle));
  GtkWindow* window = entry ? GTK_WINDOW(entry->data) : nullptr;
  g_list_free(windows);
  return window;
}

FlMethodResponse* handle_call(FlMethodCall* call) {
  FlValue* args = fl_method_call_get_args(call);
  GtkWindow* window = lookup_window(args);
  if (!window) return reply_error("invalid_window", "Expected a live GtkWindow");

  const char* method = fl_method_call_get_name(call);
  if (g_str_equal(method, "show") || g_str_equal(method, "focus")) {
    gtk_window_present(window);
  } else if (g_str_equal(method, "hide")) {
    gtk_widget_hide(GTK_WIDGET(window));
  } else if (g_str_equal(method, "isHidden")) {
    g_autoptr(FlValue) result = fl_value_new_bool(!gtk_widget_get_visible(GTK_WIDGET(window)));
    return reply_ok(result);
  } else if (g_str_equal(method, "getFrame")) {
    int x, y, width, height;
    gtk_window_get_position(window, &x, &y);
    gtk_window_get_size(window, &width, &height);
    g_autoptr(FlValue) result = fl_value_new_map();
    fl_value_set_string_take(result, "x", fl_value_new_float(x));
    fl_value_set_string_take(result, "y", fl_value_new_float(y));
    fl_value_set_string_take(result, "width", fl_value_new_float(width));
    fl_value_set_string_take(result, "height", fl_value_new_float(height));
    return reply_ok(result);
  } else if (g_str_equal(method, "setFrame")) {
    int64_t x, y, width, height;
    constexpr auto min = std::numeric_limits<int>::min();
    constexpr auto max = std::numeric_limits<int>::max();
    if (!read_int(args, "x", &x) || !read_int(args, "y", &y) ||
        !read_int(args, "width", &width) || !read_int(args, "height", &height) ||
        x < min || x > max || y < min || y > max ||
        width < 1 || width > max || height < 1 || height > max) {
      return reply_error("invalid_frame", "Expected valid GTK window bounds");
    }
    // Wayland compositors can ignore absolute positioning; resizing still works.
    gtk_window_move(window, static_cast<int>(x), static_cast<int>(y));
    gtk_window_resize(window, static_cast<int>(width), static_cast<int>(height));
  } else if (g_str_equal(method, "center")) {
    gtk_window_set_position(window, GTK_WIN_POS_CENTER);
  } else if (g_str_equal(method, "showTitleBar")) {
    int64_t show;
    if (!read_int(args, "show", &show) || (show != 0 && show != 1)) {
      return reply_error("invalid_argument", "Expected a title bar visibility flag");
    }
    gtk_window_set_decorated(window, show == 1);
  } else {
    return FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }
  return reply_ok();
}
}  // namespace

void register_official_window_controls(FlPluginRegistrar* registrar) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel = fl_method_channel_new(
      fl_plugin_registrar_get_messenger(registrar),
      "rustdesk/desktop_multi_window/linux", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      channel,
      [](FlMethodChannel*, FlMethodCall* call, gpointer) {
        g_autoptr(FlMethodResponse) response = handle_call(call);
        fl_method_call_respond(call, response, nullptr);
      },
      nullptr, nullptr);
}

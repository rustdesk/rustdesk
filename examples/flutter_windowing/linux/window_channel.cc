//
// Created by boyan on 2022/1/27.
//

#include "window_channel.h"

#include "gtk/gtk.h"

namespace {

struct MethodInvokeAsyncUserData {
 public:

  MethodInvokeAsyncUserData(FlMethodChannel *channel, FlMethodCall *method_call)
      : channel(channel), method_call(method_call) {
    g_object_ref(channel);
    g_object_ref(method_call);
  }

  ~MethodInvokeAsyncUserData() {
    g_object_unref(channel);
    g_object_unref(method_call);
  }

  FlMethodChannel *channel;
  FlMethodCall *method_call;

};

}

WindowChannel::WindowChannel(int64_t window_id, FlMethodChannel *method_channel)
    : window_id_(window_id), fl_method_channel_(method_channel) {
  g_object_ref(fl_method_channel_);
}

WindowChannel::~WindowChannel() {
  g_object_unref(fl_method_channel_);
}

// static
std::unique_ptr<WindowChannel> WindowChannel::RegisterWithRegistrar(FlPluginRegistrar *registrar, int64_t window_id) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel =
      fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),
                            "mixin.one/flutter_multi_window_channel",
                            FL_METHOD_CODEC(codec));
  auto window_channel = std::make_unique<WindowChannel>(window_id, channel);
  fl_method_channel_set_method_call_handler(
      channel,
      [](FlMethodChannel *channel, FlMethodCall *call, gpointer user_data) {
        auto *window_channel = static_cast<WindowChannel *>(user_data);
        g_assert(window_channel);

        if (!window_channel->handler_) {
          fl_method_call_respond_error(call, "-1", "window channel no handler.", nullptr, nullptr);
          return;
        }

        auto method_name = fl_method_call_get_name(call);
        auto args = fl_method_call_get_args(call);
        auto target_window_id = fl_value_get_int(fl_value_lookup_string(args, "targetWindowId"));
        auto arguments = fl_value_lookup_string(args, "arguments");

        window_channel->handler_(window_channel->window_id_, target_window_id, method_name, arguments, call);
      },
      window_channel.get(),
      nullptr);
  return window_channel;
}

void WindowChannel::InvokeMethod(
    int64_t from_window_id,
    const gchar *method,
    FlValue *arguments,
    FlMethodCall *method_call
) {
  g_autoptr(FlValue) args = fl_value_new_map();
  fl_value_set(args, fl_value_new_string("arguments"), arguments);
  fl_value_set(args, fl_value_new_string("fromWindowId"), fl_value_new_int(from_window_id));
  auto *user_data = new MethodInvokeAsyncUserData(fl_method_channel_, method_call);

  fl_method_channel_invoke_method(
      fl_method_channel_, method, args, nullptr,
      +[](GObject *source_object,
          GAsyncResult *res,
          gpointer user_data) {
        auto data = static_cast<MethodInvokeAsyncUserData *>(user_data);
        GError *error = nullptr;
        auto result = fl_method_channel_invoke_method_finish(data->channel, res, &error);
        if (error != nullptr) {
          g_critical("failed to get method finish response: %s", error->message);
        }
        fl_method_call_respond(data->method_call, result, nullptr);
        delete data;
      }, user_data);
}


void WindowChannel::InvokeMethodSelfVoid(const gchar* method, FlValue *arguments) {
  g_autoptr(FlValue) args = fl_value_new_map();
  fl_value_set(args, fl_value_new_string("arguments"), arguments);
  fl_value_set(args, fl_value_new_string("fromWindowId"), fl_value_new_int(0));
  fl_method_channel_invoke_method(
          fl_method_channel_, method, args, nullptr, nullptr, nullptr);
}
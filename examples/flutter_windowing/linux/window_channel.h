//
// Created by boyan on 2022/1/27.
//

#ifndef DESKTOP_MULTI_WINDOW_LINUX_WINDOW_CHANNEL_H_
#define DESKTOP_MULTI_WINDOW_LINUX_WINDOW_CHANNEL_H_

#include <memory>
#include <functional>

#include "flutter_linux/flutter_linux.h"

class WindowChannel {

 public:

  static std::unique_ptr<WindowChannel> RegisterWithRegistrar(FlPluginRegistrar *registrar, int64_t window_id);

  WindowChannel(int64_t window_id, FlMethodChannel *method_channel);

  ~WindowChannel();

  using MethodHandler = std::function<void(
      int64_t from_window_id,
      int64_t target_window_id,
      const gchar *method,
      FlValue *arguments,
      FlMethodCall *method_call)>;

  void SetMethodHandler(MethodHandler handler) {
    handler_ = std::move(handler);
  }

  void InvokeMethod(int64_t from_window_id,
                    const gchar *method,
                    FlValue *arguments,
                    FlMethodCall *method_call
  );

  void InvokeMethodSelfVoid(const gchar* method, FlValue *arguments);

 private:

  int64_t window_id_;
  FlMethodChannel *fl_method_channel_;
  MethodHandler handler_;

};

#endif //DESKTOP_MULTI_WINDOW_LINUX_WINDOW_CHANNEL_H_

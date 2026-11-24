//
// Created by yangbin on 2022/1/27.
//

#ifndef DESKTOP_MULTI_WINDOW_WINDOW_CHANNEL_H
#define DESKTOP_MULTI_WINDOW_WINDOW_CHANNEL_H

#include <cstdint>
#include <memory>

#include "flutter/event_channel.h"
#include "flutter/plugin_registrar.h"
#include "flutter/plugin_registrar_windows.h"
#include "flutter/method_channel.h"
#include "flutter/encodable_value.h"

class WindowChannel : public flutter::Plugin {

public:

    using Argument = flutter::EncodableValue;

    static std::unique_ptr<WindowChannel>
    RegisterWithRegistrar(FlutterDesktopPluginRegistrarRef registrar, int64_t window_id);

    WindowChannel(int64_t window_id, std::unique_ptr<flutter::MethodChannel<Argument>> channel);

    ~WindowChannel() override;

    void InvokeMethod(
            int64_t from_window_id,
            const std::string &method,
            Argument *arguments,
            std::unique_ptr<flutter::MethodResult<Argument>> result = nullptr
    );

    using MethodCallHandler = std::function<void(
            int64_t from_window_id,
            int64_t target_window_id,
            const std::string &call,
            Argument *arguments,
            std::unique_ptr<flutter::MethodResult<Argument>> result)>;

    void SetMethodCallHandler(MethodCallHandler handler) {
        handler_ = std::move(handler);
    }

private:
    int64_t window_id_;

    MethodCallHandler handler_;

    std::unique_ptr<flutter::MethodChannel<Argument>> channel_;

};


#endif //DESKTOP_MULTI_WINDOW_WINDOW_CHANNEL_H

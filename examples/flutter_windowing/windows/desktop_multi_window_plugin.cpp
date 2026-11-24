#include "include/desktop_multi_window/desktop_multi_window_plugin.h"
#include "multi_window_plugin_internal.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

#include <map>
#include <memory>

#include "multi_window_manager.h"

namespace {

class DesktopMultiWindowPlugin : public flutter::Plugin {
 public:
  static void RegisterWithRegistrar(flutter::PluginRegistrarWindows *registrar);

  DesktopMultiWindowPlugin();

  ~DesktopMultiWindowPlugin() override;

 private:
  void HandleMethodCall(
      const flutter::MethodCall<flutter::EncodableValue> &method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
};

// static
void DesktopMultiWindowPlugin::RegisterWithRegistrar(
    flutter::PluginRegistrarWindows *registrar) {
  auto channel =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          registrar->messenger(), "mixin.one/flutter_multi_window",
          &flutter::StandardMethodCodec::GetInstance());

  auto plugin = std::make_unique<DesktopMultiWindowPlugin>();

  channel->SetMethodCallHandler(
      [plugin_pointer = plugin.get()](const auto &call, auto result) {
        plugin_pointer->HandleMethodCall(call, std::move(result));
      });
  registrar->AddPlugin(std::move(plugin));
}

DesktopMultiWindowPlugin::DesktopMultiWindowPlugin() = default;

DesktopMultiWindowPlugin::~DesktopMultiWindowPlugin() = default;

void DesktopMultiWindowPlugin::HandleMethodCall(
    const flutter::MethodCall<flutter::EncodableValue> &method_call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  if (method_call.method_name() == "createWindow") {
    auto args = std::get_if<std::string>(method_call.arguments());
    auto window_id = MultiWindowManager::Instance()->Create(*args);
    result->Success(flutter::EncodableValue(window_id));
    return;
  } else if (method_call.method_name() == "show") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Show(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "hide") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Hide(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "isHidden") {
    auto window_id = method_call.arguments()->LongValue();
    auto res = MultiWindowManager::Instance()->IsHidden(window_id);
    result->Success(flutter::EncodableValue(res));
    return;
  } else if (method_call.method_name() == "close") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Close(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "setFrame") {
    auto *arguments = std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id = arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto left = std::get<double_t>(arguments->at(flutter::EncodableValue("left")));
    auto top = std::get<double_t>(arguments->at(flutter::EncodableValue("top")));
    auto width = std::get<double_t>(arguments->at(flutter::EncodableValue("width")));
    auto height = std::get<double_t>(arguments->at(flutter::EncodableValue("height")));
    MultiWindowManager::Instance()->SetFrame(window_id, left, top, width, height);
    result->Success();
    return;
  } else if (method_call.method_name() == "getFrame") {
    auto *arguments = std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id = arguments->at(flutter::EncodableValue("windowId")).LongValue();
    flutter::EncodableMap value = MultiWindowManager::Instance()->GetFrame(window_id);
    result->Success(flutter::EncodableValue(value));
    return;
  } else if (method_call.method_name() == "center") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Center(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "setTitle") {
    auto *arguments = std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id = arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto title = std::get<std::string>(arguments->at(flutter::EncodableValue("title")));
    MultiWindowManager::Instance()->SetTitle(window_id, title);
    result->Success();
    return;
  } else if (method_call.method_name() == "getAllSubWindowIds") {
    auto window_ids = MultiWindowManager::Instance()->GetAllSubWindowIds();
    // window_ids must contain at least one element
    // It seems a bug of flutter engine
    if (window_ids.empty()) {
      window_ids.push_back(0);
    }
    result->Success(flutter::EncodableValue(window_ids));
    return;
  } else if (method_call.method_name() == "focus") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Focus(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "setFullscreen") {
    auto *arguments = std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id = arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto fullscreen = std::get<bool>(arguments->at(flutter::EncodableValue("fullscreen")));
    MultiWindowManager::Instance()->SetFullscreen(window_id, fullscreen);
    result->Success();
    return;
  } else if (method_call.method_name() == "isFullscreen") {
    auto *arguments =
        std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id =
        arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto isFullScreen = MultiWindowManager::Instance()->IsFullscreen(window_id);
    result->Success(flutter::EncodableValue(isFullScreen));
    return;
  } else if (method_call.method_name() == "startDragging") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->StartDragging(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "maximize") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Maximize(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "minimize") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Minimize(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "unmaximize") {
    auto window_id = method_call.arguments()->LongValue();
    MultiWindowManager::Instance()->Unmaximize(window_id);
    result->Success();
    return;
  } else if (method_call.method_name() == "isMaximized") {
    auto window_id = method_call.arguments()->LongValue();
    auto res = MultiWindowManager::Instance()->IsMaximized(window_id);
    result->Success(flutter::EncodableValue(res));
    return;
  } else if (method_call.method_name() == "isMinimized") {
    auto window_id = method_call.arguments()->LongValue();
    auto res = MultiWindowManager::Instance()->IsMinimized(window_id);
    result->Success(flutter::EncodableValue(res));
    return;
  } else if (method_call.method_name() == "showTitleBar") {
    auto *arguments =
        std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id =
        arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto show = std::get<bool>(arguments->at(flutter::EncodableValue("show")));
    MultiWindowManager::Instance()->ShowTitlebar(window_id, show);
    result->Success();
    return;
  } else if (method_call.method_name() == "startResizing") {
    auto *arguments =
        std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id =
        arguments->at(flutter::EncodableValue("windowId")).LongValue();
    MultiWindowManager::Instance()->StartResizing(window_id, arguments);
    result->Success();
    return;
  } else if (method_call.method_name() == "setPreventClose") {
    auto *arguments =
        std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id =
        arguments->at(flutter::EncodableValue("windowId")).LongValue();
    auto setPreventClose = std::get<bool>(arguments->at(flutter::EncodableValue("setPreventClose")));
    MultiWindowManager::Instance()->SetPreventClose(window_id, setPreventClose);
    result->Success();
    return;
  } else if (method_call.method_name() == "isPreventClose") {
    auto window_id = method_call.arguments()->LongValue();
    auto res = MultiWindowManager::Instance()->IsPreventClose(window_id);
    result->Success(flutter::EncodableValue(res));
    return;
  } else if (method_call.method_name() == "setInitBackgroundColor") {
    auto *arguments = std::get_if<flutter::EncodableMap>(method_call.arguments());
    auto window_id = arguments->at(flutter::EncodableValue("windowId")).LongValue();
    MultiWindowManager::Instance()->SetInitBackgroundColor(window_id, arguments);
    return result->Success();
  }
  result->NotImplemented();
}

}  // namespace

void DesktopMultiWindowPluginRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {

  InternalMultiWindowPluginRegisterWithRegistrar(registrar);

  // Attach MainWindow for
  auto hwnd = FlutterDesktopViewGetHWND(FlutterDesktopPluginRegistrarGetView(registrar));
  auto channel = WindowChannel::RegisterWithRegistrar(registrar, 0);
  MultiWindowManager::Instance()->AttachFlutterMainWindow(GetAncestor(hwnd, GA_ROOT),
                                                          std::move(channel));
}

void InternalMultiWindowPluginRegisterWithRegistrar(FlutterDesktopPluginRegistrarRef registrar) {
  DesktopMultiWindowPlugin::RegisterWithRegistrar(
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar));
}
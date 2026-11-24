#include "include/desktop_multi_window/desktop_multi_window_plugin.h"
#include "official_window_controls.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

// This harness deliberately cannot start a legacy engine by accident.
void DesktopMultiWindowPluginRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  RegisterOfficialWindowControls(registrar);
  auto* plugin_registrar = flutter::PluginRegistrarManager::GetInstance()
      ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);
  flutter::MethodChannel<flutter::EncodableValue> channel(
      plugin_registrar->messenger(), "mixin.one/flutter_multi_window",
      &flutter::StandardMethodCodec::GetInstance());
  channel.SetMethodCallHandler([](const auto&, auto result) {
    result->Error("official_only", "Install ExperimentalWindowingBackend first");
  });
}

void DesktopMultiWindowSetWindowCreatedCallback(WindowCreatedCallback) {}

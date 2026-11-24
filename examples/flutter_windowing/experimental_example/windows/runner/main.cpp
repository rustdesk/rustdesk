#include <flutter/dart_project.h>
#include <flutter/flutter_engine.h>
#include <windows.h>
#include <algorithm>
#include <chrono>
#include "flutter/generated_plugin_registrant.h"
#include "utils.h"

int APIENTRY wWinMain(HINSTANCE, HINSTANCE, wchar_t*, int) {
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  {
    flutter::DartProject project(L"data");
    project.set_dart_entrypoint_arguments(GetCommandLineArguments());
    flutter::FlutterEngine engine(project);
    if (!engine.Run()) return EXIT_FAILURE;
    RegisterPlugins(&engine);
    bool running = true;
    while (running) {
      const auto delay = std::chrono::duration_cast<std::chrono::milliseconds>(engine.ProcessMessages()).count();
      const DWORD timeout = static_cast<DWORD>(std::clamp<int64_t>(delay, 0, 100));
      ::MsgWaitForMultipleObjectsEx(0, nullptr, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
      MSG message;
      while (::PeekMessage(&message, nullptr, 0, 0, PM_REMOVE)) {
        if (message.message == WM_QUIT) { running = false; break; }
        ::TranslateMessage(&message);
        ::DispatchMessage(&message);
      }
    }
  }
  ::CoUninitialize();
  return EXIT_SUCCESS;
}

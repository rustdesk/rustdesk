#include <flutter/dart_project.h>
#include <flutter/flutter_engine.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>
#include <algorithm>
#include <chrono>
#include "flutter/generated_plugin_registrant.h"
#include "utils.h"

namespace {
void PumpMessages(flutter::FlutterEngine& engine) {
  const bool self_test = GetEnvironmentVariableW(L"POC_RESULTS", nullptr, 0) != 0;
  while (true) {
    const auto delay = std::chrono::duration_cast<std::chrono::milliseconds>(
        engine.ProcessMessages()).count();
    const DWORD timeout = static_cast<DWORD>(std::clamp<int64_t>(delay, 0, 100));
    ::MsgWaitForMultipleObjectsEx(0, nullptr, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    MSG message;
    while (::PeekMessage(&message, nullptr, 0, 0, PM_REMOVE)) {
      if (message.message == WM_QUIT) return;
      // Record the move request without entering the OS modal drag loop,
      // which requires an interactive mouse and an unlocked desktop.
      if (self_test && message.message == WM_SYSCOMMAND &&
          message.wParam == (SC_MOVE | HTCAPTION)) {
        SetPropW(message.hwnd, L"RustDeskPocMoveRequested",
                 reinterpret_cast<HANDLE>(message.wParam));
        continue;
      }
      ::TranslateMessage(&message);
      ::DispatchMessage(&message);
    }
  }
}

int RunWithImplicitView(const flutter::DartProject& project) {
  // The regular RustDesk runner embeds an implicit FlutterView in its HWND.
  // Keep that view present while the experiment opens additional SDK windows.
  HWND host = CreateWindowExW(0, L"STATIC", L"RustDesk official windowing PoC",
                              WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                              720, 500, nullptr, nullptr, GetModuleHandleW(nullptr),
                              nullptr);
  if (!host) return EXIT_FAILURE;
  {
    RECT bounds{};
    GetClientRect(host, &bounds);
    flutter::FlutterViewController controller(bounds.right, bounds.bottom, project);
    if (!controller.engine() || !controller.view()) {
      DestroyWindow(host);
      return EXIT_FAILURE;
    }
    RegisterPlugins(controller.engine());
    HWND view = controller.view()->GetNativeWindow();
    SetParent(view, host);
    MoveWindow(view, 0, 0, bounds.right, bounds.bottom, TRUE);
    SetWindowPos(host, nullptr, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW);
    PumpMessages(*controller.engine());
  }
  DestroyWindow(host);
  return EXIT_SUCCESS;
}
}  // namespace

int APIENTRY wWinMain(HINSTANCE, HINSTANCE, wchar_t*, int) {
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  int result = EXIT_FAILURE;
  {
    flutter::DartProject project(L"data");
    const auto args = GetCommandLineArguments();
    project.set_dart_entrypoint_arguments(args);
    if (std::find(args.begin(), args.end(), "--implicit-main-self-test") != args.end()) {
      result = RunWithImplicitView(project);
    } else {
      flutter::FlutterEngine engine(project);
      if (engine.Run()) {
        RegisterPlugins(&engine);
        PumpMessages(engine);
        result = EXIT_SUCCESS;
      }
    }
  }
  ::CoUninitialize();
  return result;
}

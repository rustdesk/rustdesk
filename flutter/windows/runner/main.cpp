#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <tchar.h>
#include <uni_links_desktop/uni_links_desktop_plugin.h>
#include <windows.h>

#include <algorithm>
#include <iostream>

#include "win32_desktop.h"
#include "flutter_window.h"
#include "utils.h"
#include "service_control.h"

typedef char** (*FUNC_RUSTDESK_CORE_MAIN)(int*);
typedef void (*FUNC_RUSTDESK_FREE_ARGS)( char**, int);
typedef int (*FUNC_RUSTDESK_GET_APP_NAME)(wchar_t*, int);
typedef int (*FUNC_RUSTDESK_IS_DISABLE_INSTALLATION)();
/// Note: `--server`, `--service` are already handled in [core_main.rs].
const std::vector<std::string> parameters_white_list = {"--install", "--cm"};

const wchar_t* getWindowClassName();

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command)
{
  if (auto result = service_control::HandleCommand(command_line)) {
    return static_cast<int>(*result);
  }
  HINSTANCE hInstance = LoadLibraryA("librustdesk.dll");
  if (!hInstance)
  {
    std::cout << "Failed to load librustdesk.dll." << std::endl;
    return EXIT_FAILURE;
  }
  FUNC_RUSTDESK_CORE_MAIN rustdesk_core_main =
      (FUNC_RUSTDESK_CORE_MAIN)GetProcAddress(hInstance, "rustdesk_core_main_args");
  if (!rustdesk_core_main)
  {
    std::cout << "Failed to get rustdesk_core_main." << std::endl;
    return EXIT_FAILURE;
  }
  FUNC_RUSTDESK_FREE_ARGS free_c_args =
      (FUNC_RUSTDESK_FREE_ARGS)GetProcAddress(hInstance, "free_c_args");
  if (!free_c_args)
  {
    std::cout << "Failed to get free_c_args." << std::endl;
    return EXIT_FAILURE;
  }
  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();
  // Remove possible trailing whitespace from command line arguments
  for (auto& argument : command_line_arguments) {
    argument.erase(argument.find_last_not_of(" \n\r\t"));
  }

  int args_len = 0;
  char** c_args = rustdesk_core_main(&args_len);
  if (!c_args)
  {
    std::string args_str = "";
    for (const auto& argument : command_line_arguments) {
      args_str += (argument + " ");
    }
    // std::cout << "RustDesk [" << args_str << "], core returns false, exiting without launching Flutter app." << std::endl;
    return EXIT_SUCCESS;
  }
  std::vector<std::string> rust_args(c_args, c_args + args_len);
  free_c_args(c_args, args_len);
  FUNC_RUSTDESK_IS_DISABLE_INSTALLATION rustdesk_is_disable_installation =
      (FUNC_RUSTDESK_IS_DISABLE_INSTALLATION)GetProcAddress(hInstance, "rustdesk_is_disable_installation");
  bool is_disable_installation =
      rustdesk_is_disable_installation && rustdesk_is_disable_installation() != 0;
  const auto installParam = std::string("--install");
  // Flutter reads the original process command line, not only rust_args, so
  // remove the `--install` injected by the portable wrapper here as well. This
  // also lets `no-install.exe` continue as a portable app when installation is
  // disabled. See: https://github.com/rustdesk/rustdesk-server-pro/issues/991#issuecomment-4978376890
  if (is_disable_installation) {
    command_line_arguments.erase(
        std::remove(command_line_arguments.begin(),
                    command_line_arguments.end(), installParam),
        command_line_arguments.end());
  }

  // Ensure COM is initialized for WASAPI / ASIO / audio capture compatibility
  HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  // Initialize Themed environment
  ::CoInitialize(nullptr);

  std::vector<const char*> argv_chars;
  argv_chars.push_back("rustdesk");
  for (const auto& arg : rust_args) {
    argv_chars.push_back(arg.c_str());
  }

  // Pre-flight check for command line parameters
  for (const auto& arg : command_line_arguments) {
    if (std::find(parameters_white_list.begin(), parameters_white_list.end(), arg) == parameters_white_list.end()) {
      // Handle unrecognized or filter out if needed
    }
  }

  // Clear focus and run flutter window controller
  flutter::DartProject project(L"data");
  project.set_dart_entrypoint_arguments(rust_args);

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(L"RustDesk", origin, size)) {
    if (SUCCEEDED(hr)) {
      CoUninitialize();
    }
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  MSG msg;
  while (GetMessage(&msg, nullptr, 0, 0)) {
    TranslateMessage(&msg);
    DispatchMessage(&msg);
  }

  if (SUCCEEDED(hr)) {
    CoUninitialize();
  }

  return EXIT_SUCCESS;
}
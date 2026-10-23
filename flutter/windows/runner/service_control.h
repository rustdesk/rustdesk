#ifndef RUSTDESK_RUNNER_SERVICE_CONTROL_H_
#define RUSTDESK_RUNNER_SERVICE_CONTROL_H_

#include <windows.h>
#include <shellapi.h>
#include <winsvc.h>

#include <cstdio>
#include <cwchar>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>

namespace service_control {

using ServiceHandle = std::unique_ptr<std::remove_pointer_t<SC_HANDLE>,
                                      decltype(&CloseServiceHandle)>;
constexpr DWORD kStopTimeoutMilliseconds = 30000;
constexpr DWORD kPollIntervalMilliseconds = 100;

inline DWORD StopService(SC_HANDLE service) {
  SERVICE_STATUS_PROCESS status{};
  DWORD bytes_needed = 0;
  if (!QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO,
                            reinterpret_cast<BYTE*>(&status), sizeof(status),
                            &bytes_needed)) {
    return GetLastError();
  }
  if (status.dwCurrentState == SERVICE_STOPPED) {
    return ERROR_SUCCESS;
  }
  if (status.dwCurrentState != SERVICE_STOP_PENDING) {
    SERVICE_STATUS stopped{};
    if (!ControlService(service, SERVICE_CONTROL_STOP, &stopped)) {
      const DWORD error = GetLastError();
      return error == ERROR_SERVICE_NOT_ACTIVE ? ERROR_SUCCESS : error;
    }
  }
  // The installer may remove files next; accepting STOP alone does not mean
  // the service has reached SERVICE_STOPPED.
  const ULONGLONG started = GetTickCount64();
  while (status.dwCurrentState != SERVICE_STOPPED) {
    if (GetTickCount64() - started >= kStopTimeoutMilliseconds) {
      return ERROR_TIMEOUT;
    }
    Sleep(kPollIntervalMilliseconds);
    if (!QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO,
                              reinterpret_cast<BYTE*>(&status), sizeof(status),
                              &bytes_needed)) {
      return GetLastError();
    }
  }
  return ERROR_SUCCESS;
}

inline DWORD RemoveService(const wchar_t* name) {
  ServiceHandle manager(OpenSCManagerW(nullptr, nullptr, SC_MANAGER_CONNECT),
                        CloseServiceHandle);
  if (!manager) {
    return GetLastError();
  }
  ServiceHandle service(
      OpenServiceW(manager.get(), name,
                   SERVICE_STOP | SERVICE_QUERY_STATUS | DELETE),
      CloseServiceHandle);
  if (!service) {
    const DWORD error = GetLastError();
    return error == ERROR_SERVICE_DOES_NOT_EXIST ? ERROR_SUCCESS : error;
  }
  const DWORD stopped = StopService(service.get());
  if (stopped != ERROR_SUCCESS) {
    // The uninstall script must still reach its taskkill cleanup.
    std::fwprintf(stderr,
                  L"Service did not stop cleanly: Windows error %lu\n", stopped);
  }
  if (DeleteService(service.get())) {
    return ERROR_SUCCESS;
  }
  const DWORD error = GetLastError();
  return error == ERROR_SERVICE_MARKED_FOR_DELETE ? ERROR_SUCCESS : error;
}

inline DWORD InstallService(const wchar_t* name, const wchar_t* executable,
                            bool rollback_on_failure) {
  if (!*executable || wcschr(executable, L'"')) {
    return ERROR_INVALID_PARAMETER;
  }
  const std::wstring binary_path =
      L"\"" + std::wstring(executable) + L"\" --service";
  const std::wstring display_name = std::wstring(name) + L" Service";
  ServiceHandle manager(
      OpenSCManagerW(nullptr, nullptr,
                     SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE),
      CloseServiceHandle);
  if (!manager) {
    return GetLastError();
  }
  ServiceHandle service(
      CreateServiceW(manager.get(), name, display_name.c_str(),
                     SERVICE_START | DELETE, SERVICE_WIN32_OWN_PROCESS,
                     SERVICE_AUTO_START, SERVICE_ERROR_NORMAL,
                     binary_path.c_str(), nullptr,
                     nullptr, nullptr, nullptr, nullptr),
      CloseServiceHandle);
  const bool created = service != nullptr;
  // --after-install can reuse a registration. Preserve its executable and
  // account, as the previous create-then-start script did.
  if (!service && GetLastError() == ERROR_SERVICE_EXISTS) {
    service.reset(OpenServiceW(manager.get(), name, SERVICE_START));
  }
  if (!service) {
    return GetLastError();
  }
  if (StartServiceW(service.get(), 0, nullptr)) {
    return ERROR_SUCCESS;
  }
  const DWORD error = GetLastError();
  // Only manual Start Service restores stop-service=Y on failure. Other
  // installation paths retain the registration for a later startup attempt.
  if (rollback_on_failure && created && error != ERROR_SERVICE_ALREADY_RUNNING &&
      !DeleteService(service.get())) {
    const DWORD rollback_error = GetLastError();
    std::fwprintf(stderr,
                  L"Failed to roll back newly created service: Windows error %lu\n",
                  rollback_error);
  }
  return error == ERROR_SERVICE_ALREADY_RUNNING ? ERROR_SUCCESS : error;
}

inline DWORD RunCommand(int count, wchar_t** args) {
  constexpr int kRemoveArgumentCount = 4;
  constexpr int kInstallArgumentCount = 5;
  if (count < kRemoveArgumentCount) {
    return ERROR_BAD_ARGUMENTS;
  }
  if (!*args[3]) {
    return ERROR_INVALID_PARAMETER;
  }
  const std::wstring_view operation(args[2]);
  const bool rollback_on_failure = operation == L"create-service-with-rollback";
  if ((operation == L"create-service" || rollback_on_failure) &&
      count == kInstallArgumentCount) {
    return InstallService(args[3], args[4], rollback_on_failure);
  }
  if (operation == L"remove" && count == kRemoveArgumentCount) {
    return RemoveService(args[3]);
  }
  return ERROR_BAD_ARGUMENTS;
}

// Keep these commands in the existing runner so installation/removal can control
// services before loading librustdesk.dll or initializing the UI. A Rust-side
// handler would first require that DLL to load. The header-only implementation
// keeps main.cpp as a dispatch hook without adding a CMake source entry; it is
// compiled into the existing EXE, not a separately packaged helper.
inline std::optional<DWORD> HandleCommand(const wchar_t* command_line) {
  constexpr std::wstring_view option = L"--service-control";
  const std::wstring_view arguments(command_line);
  if (arguments.compare(0, option.size(), option) != 0 ||
      (arguments.size() > option.size() && arguments[option.size()] != L' ' &&
       arguments[option.size()] != L'\t')) {
    return std::nullopt;
  }
  int count = 0;
  wchar_t** args = CommandLineToArgvW(GetCommandLineW(), &count);
  if (!args) {
    return GetLastError();
  }
  const DWORD result = RunCommand(count, args);
  LocalFree(args);
  if (result != ERROR_SUCCESS) {
    std::fwprintf(stderr,
                  L"RustDesk service control failed with Windows error %lu\n",
                  result);
  }
  return result;
}

}  // namespace service_control

#endif  // RUSTDESK_RUNNER_SERVICE_CONTROL_H_

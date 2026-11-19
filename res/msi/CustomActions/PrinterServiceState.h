#pragma once

#include <Windows.h>
#include <memory>
#include <vector>

struct PrinterServiceState {
    int current = 0;
    int startType = 0;
};

inline HRESULT ReadPrinterServiceState(LPCWSTR name, PrinterServiceState& state) {
    using Service = std::unique_ptr<SC_HANDLE__, decltype(&CloseServiceHandle)>;
    Service manager(OpenSCManagerW(nullptr, nullptr, SC_MANAGER_CONNECT), CloseServiceHandle);
    if (!manager) return HRESULT_FROM_WIN32(GetLastError());
    Service service(OpenServiceW(manager.get(), name, SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG), CloseServiceHandle);
    if (!service) {
        const DWORD error = GetLastError();
        return error == ERROR_SERVICE_DOES_NOT_EXIST ? S_OK : HRESULT_FROM_WIN32(error);
    }
    SERVICE_STATUS_PROCESS status{};
    DWORD needed = 0;
    if (!QueryServiceStatusEx(service.get(), SC_STATUS_PROCESS_INFO,
        reinterpret_cast<BYTE*>(&status), sizeof(status), &needed))
        return HRESULT_FROM_WIN32(GetLastError());
    if (QueryServiceConfigW(service.get(), nullptr, 0, &needed)) return E_UNEXPECTED;
    if (GetLastError() != ERROR_INSUFFICIENT_BUFFER) return HRESULT_FROM_WIN32(GetLastError());
    std::vector<BYTE> bytes(needed);
    const auto config = reinterpret_cast<QUERY_SERVICE_CONFIGW*>(bytes.data());
    if (!QueryServiceConfigW(service.get(), config, needed, &needed))
        return HRESULT_FROM_WIN32(GetLastError());
    state.current = static_cast<int>(status.dwCurrentState);
    state.startType = static_cast<int>(config->dwStartType);
    return S_OK;
}
